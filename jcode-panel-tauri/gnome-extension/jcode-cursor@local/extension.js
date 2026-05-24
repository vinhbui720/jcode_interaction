import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'org.jcode.Panel.Cursor';
const OBJECT_PATH = '/org/jcode/Panel/Cursor';

const CursorIface = `<node>
  <interface name="org.jcode.Panel.Cursor">
    <method name="GetPosition">
      <arg type="i" name="x" direction="out"/>
      <arg type="i" name="y" direction="out"/>
    </method>
  </interface>
</node>`;

export default class JcodeCursorExtension extends Extension {
    constructor(metadata) {
        super(metadata);
        this._ownerId = 0;
        this._dbus = null;
    }

    GetPosition() {
        const [x, y] = global.get_pointer();
        return [x, y];
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
