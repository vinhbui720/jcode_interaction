'use strict';

const Clutter = imports.gi.Clutter;
const GLib = imports.gi.GLib;
const Gio = imports.gi.Gio;

const BUS_NAME = 'org.jcode.Panel.Cursor';
const OBJECT_PATH = '/org/jcode/Panel/Cursor';
const TRIGGER_COOLDOWN_MS = 450;

const CursorIface = `<node>
  <interface name="org.jcode.Panel.Cursor">
    <method name="GetPosition">
      <arg type="i" name="x" direction="out"/>
      <arg type="i" name="y" direction="out"/>
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
    } catch (_error) {
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
    }

    GetPosition() {
        let [x, y] = global.get_pointer();
        return [x, y];
    }

    _handleCapturedEvent(_actor, event) {
        if (event.type() !== Clutter.EventType.BUTTON_PRESS)
            return Clutter.EVENT_PROPAGATE;

        const [, key] = hotkeyParts(readPromptHotkey());
        if (!key.startsWith('mouse'))
            return Clutter.EVENT_PROPAGATE;

        const wantedButton = Number(key.slice('mouse'.length));
        if (!wantedButton || event.get_button() !== wantedButton)
            return Clutter.EVENT_PROPAGATE;

        const [modifiers] = hotkeyParts(readPromptHotkey());
        if (!stateHasModifiers(event.get_state(), modifiers))
            return Clutter.EVENT_PROPAGATE;

        const now = GLib.get_monotonic_time() / 1000;
        if (now - this._lastTriggerMs < TRIGGER_COOLDOWN_MS)
            return Clutter.EVENT_STOP;
        this._lastTriggerMs = now;

        try {
            GLib.spawn_command_line_async(promptCommand());
        } catch (error) {
            log(`jcode-cursor failed to launch prompt: ${error}`);
        }
        return Clutter.EVENT_STOP;
    }

    enable() {
        this._dbus = Gio.DBusExportedObject.wrapJSObject(CursorIface, this);
        this._ownerId = Gio.bus_own_name(
            Gio.BusType.SESSION,
            BUS_NAME,
            Gio.BusNameOwnerFlags.NONE,
            connection => {
                if (this._dbus)
                    this._dbus.export(connection, OBJECT_PATH);
            },
            null,
            null
        );
        this._eventId = global.stage.connect('captured-event', this._handleCapturedEvent.bind(this));
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
    }
}

function init() {
    return new Extension();
}
