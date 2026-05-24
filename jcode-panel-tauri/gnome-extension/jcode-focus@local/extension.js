import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'org.jcode.Panel.Focus';
const OBJECT_PATH = '/org/jcode/Panel/Focus';
const PROMPT_TITLE = 'Jcode Prompt';

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
                    this._lastFocusedTitle = title;
                    win.activate(global.get_current_time());
                    return true;
                }
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
