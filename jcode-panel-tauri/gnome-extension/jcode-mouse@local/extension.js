'use strict';

const Clutter = imports.gi.Clutter;
const GLib = imports.gi.GLib;
const Gio = imports.gi.Gio;

const BUS_NAME = 'org.jcode.Panel.MouseHotkey';
const OBJECT_PATH = '/org/jcode/Panel/MouseHotkey';
const TRIGGER_COOLDOWN_MS = 450;
const PROMPT_TITLE = 'Jcode Prompt';

const MouseIface = `<node>
  <interface name="org.jcode.Panel.MouseHotkey">
    <method name="Status">
      <arg type="s" name="status" direction="out"/>
    </method>
    <method name="TriggerPrompt">
      <arg type="b" name="ok" direction="out"/>
    </method>
    <method name="FocusPrompt">
      <arg type="b" name="focused" direction="out"/>
    </method>
  </interface>
</node>`;

function normalizeKeyName(name) {
    let key = String(name || '').trim().toLowerCase().replace(/ /g, '_');
    const aliases = {
        control: 'ctrl', control_l: 'ctrl', control_r: 'ctrl', ctrl_l: 'ctrl', ctrl_r: 'ctrl',
        shift_l: 'shift', shift_r: 'shift',
        alt_l: 'alt', alt_r: 'alt', option: 'alt',
        super_l: 'super', super_r: 'super', cmd: 'super', command: 'super', win: 'super', windows: 'super',
    };
    return aliases[key] || key;
}

function hotkeyParts(hotkey) {
    const parts = String(hotkey || 'F8').replace(/-/g, '+').split('+').map(normalizeKeyName).filter(Boolean);
    const modifiers = new Set(parts.filter(part => ['ctrl', 'alt', 'shift', 'super'].includes(part)));
    const keys = parts.filter(part => !modifiers.has(part));
    return [modifiers, keys.length ? keys[keys.length - 1] : 'f8'];
}

function readPromptHotkey() {
    const path = GLib.build_filenamev([GLib.get_home_dir(), '.config', 'jcode-panel', 'config.toml']);
    try {
        const [, bytes] = GLib.file_get_contents(path);
        const text = imports.byteArray.toString(bytes);
        const match = text.match(/^prompt_hotkey\s*=\s*"([^"]+)"/m);
        return match ? match[1] : 'F8';
    } catch (error) {
        log(`jcode-mouse could not read config: ${error}`);
        return 'F8';
    }
}

function stateHasModifiers(state, modifiers) {
    if (modifiers.has('ctrl') && !(state & Clutter.ModifierType.CONTROL_MASK)) return false;
    if (modifiers.has('alt') && !(state & Clutter.ModifierType.MOD1_MASK)) return false;
    if (modifiers.has('shift') && !(state & Clutter.ModifierType.SHIFT_MASK)) return false;
    const superMask = (Clutter.ModifierType.SUPER_MASK || 0) | (Clutter.ModifierType.MOD4_MASK || 0);
    if (modifiers.has('super') && !(state & superMask)) return false;
    return true;
}

function promptCommand() {
    return GLib.build_filenamev([GLib.get_home_dir(), '.local', 'bin', 'jcp']) + ' prompt';
}

class Extension {
    constructor() {
        this._ownerId = 0;
        this._dbus = null;
        this._eventId = 0;
        this._lastTriggerMs = 0;
        this._lastButton = 0;
        this._lastButtonMs = 0;
        this._pressLogCount = 0;
    }

    Status() {
        return `hotkey=${readPromptHotkey()} eventId=${this._eventId} lastButton=${this._lastButton} lastButtonMs=${this._lastButtonMs}`;
    }

    TriggerPrompt() {
        return this._launchPrompt();
    }

    FocusPrompt() {
        return this._focusPromptWindow();
    }

    _focusPromptWindow() {
        try {
            const actors = global.get_window_actors ? global.get_window_actors() : [];
            for (const actor of actors) {
                const win = actor.meta_window;
                if (!win)
                    continue;
                const title = win.get_title ? win.get_title() : '';
                if (title !== PROMPT_TITLE)
                    continue;
                if (win.activate) {
                    win.activate(global.get_current_time());
                    return true;
                }
            }
        } catch (error) {
            log(`jcode-mouse failed to focus prompt: ${error}`);
        }
        return false;
    }

    _launchPrompt() {
        try {
            GLib.spawn_command_line_async(promptCommand());
            for (const delay of [120, 300, 700, 1200]) {
                GLib.timeout_add(GLib.PRIORITY_DEFAULT, delay, () => {
                    this._focusPromptWindow();
                    return GLib.SOURCE_REMOVE;
                });
            }
            return true;
        } catch (error) {
            log(`jcode-mouse failed to launch prompt: ${error}`);
            return false;
        }
    }

    _handleCapturedEvent(_actor, event) {
        if (event.type() !== Clutter.EventType.BUTTON_PRESS)
            return Clutter.EVENT_PROPAGATE;

        this._lastButton = event.get_button();
        this._lastButtonMs = GLib.get_monotonic_time() / 1000;
        if (this._pressLogCount < 30) {
            this._pressLogCount += 1;
            log(`jcode-mouse button press button=${this._lastButton} state=${event.get_state()} hotkey=${readPromptHotkey()}`);
        }

        const [modifiers, key] = hotkeyParts(readPromptHotkey());
        if (!key.startsWith('mouse'))
            return Clutter.EVENT_PROPAGATE;

        const wantedButton = Number(key.slice('mouse'.length));
        if (!wantedButton || event.get_button() !== wantedButton)
            return Clutter.EVENT_PROPAGATE;

        if (!stateHasModifiers(event.get_state(), modifiers))
            return Clutter.EVENT_PROPAGATE;

        const now = GLib.get_monotonic_time() / 1000;
        if (now - this._lastTriggerMs < TRIGGER_COOLDOWN_MS)
            return Clutter.EVENT_STOP;
        this._lastTriggerMs = now;

        this._launchPrompt();
        return Clutter.EVENT_STOP;
    }

    enable() {
        this._dbus = Gio.DBusExportedObject.wrapJSObject(MouseIface, this);
        this._ownerId = Gio.bus_own_name(
            Gio.BusType.SESSION,
            BUS_NAME,
            Gio.BusNameOwnerFlags.REPLACE,
            connection => {
                if (this._dbus)
                    this._dbus.export(connection, OBJECT_PATH);
            },
            null,
            null
        );
        this._eventId = global.stage.connect('captured-event', this._handleCapturedEvent.bind(this));
        log(`jcode-mouse enabled hotkey=${readPromptHotkey()} eventId=${this._eventId}`);
    }

    disable() {
        if (this._eventId) {
            global.stage.disconnect(this._eventId);
            this._eventId = 0;
        }
        if (this._dbus) {
            this._dbus.unexport();
            this._dbus = null;
        }
        if (this._ownerId) {
            Gio.bus_unown_name(this._ownerId);
            this._ownerId = 0;
        }
        log('jcode-mouse disabled');
    }
}

function init() {
    return new Extension();
}
