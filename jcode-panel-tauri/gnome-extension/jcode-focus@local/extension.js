import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'org.jcode.Panel.Focus';
const OBJECT_PATH = '/org/jcode/Panel/Focus';
const PROMPT_TITLE = 'Jcode Prompt';
const APP_CLASS_HINTS = ['jcode-panel-tauri', 'jcode interaction', 'jcode prompt'];

const FocusIface = `<node>
  <interface name="org.jcode.Panel.Focus">
    <method name="Status">
      <arg type="s" name="status" direction="out"/>
    </method>
    <method name="FocusPrompt">
      <arg type="b" name="focused" direction="out"/>
    </method>
  </interface>
</node>`;

export default class JcodeFocusExtension extends Extension {
    constructor(metadata) {
        super(metadata);
        this._ownerId = 0;
        this._dbus = null;
        this._lastFocusedTitle = '';
    }

    Status() {
        return `lastFocusedTitle=${this._lastFocusedTitle}`;
    }

    FocusPrompt() {
        return this._focusPromptWindow();
    }

    _matchesPromptWindow(win) {
        const title = String(win?.get_title ? win.get_title() : '').trim();
        const wmClass = String(win?.get_wm_class ? win.get_wm_class() : '').trim();
        const wmClassInstance = String(win?.get_wm_class_instance ? win.get_wm_class_instance() : '').trim();
        const haystacks = [title, wmClass, wmClassInstance].map(value => value.toLowerCase());
        return haystacks.some(value =>
            value.includes(PROMPT_TITLE.toLowerCase()) ||
            APP_CLASS_HINTS.some(hint => value.includes(hint))
        );
    }

    _activateWindow(win) {
        if (!win)
            return false;
        const timestamp = global.get_current_time ? global.get_current_time() : 0;
        try {
            if (win.activate)
                win.activate(timestamp);
        } catch (error) {
            console.error(`jcode-focus activate failed: ${error}`);
        }
        try {
            if (win.raise)
                win.raise();
        } catch (error) {
            console.error(`jcode-focus raise failed: ${error}`);
        }
        try {
            if (win.unminimize)
                win.unminimize(timestamp);
        } catch (error) {
            console.error(`jcode-focus unminimize failed: ${error}`);
        }
        try {
            if (win.focus)
                win.focus(timestamp);
        } catch (error) {
            console.error(`jcode-focus focus failed: ${error}`);
        }
        return true;
    }

    _focusPromptWindow() {
        try {
            const actors = global.get_window_actors ? global.get_window_actors() : [];
            for (const actor of actors) {
                const win = actor.meta_window;
                if (!win)
                    continue;
                if (!this._matchesPromptWindow(win))
                    continue;
                this._lastFocusedTitle = String(win.get_title ? win.get_title() : PROMPT_TITLE);
                return this._activateWindow(win);
            }
        } catch (error) {
            console.error(`jcode-focus failed to focus prompt: ${error}`);
        }
        return false;
    }

    enable() {
        this._dbus = Gio.DBusExportedObject.wrapJSObject(FocusIface, this);
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
    }

    disable() {
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
