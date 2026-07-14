globalThis.SwitchPreferenceCompat = class SwitchPreferenceCompat {
    constructor(context) {}
    setKey(v)          { this._key     = v; return this; }
    setTitle(v)        { this._title   = v; return this; }
    setSummary(v)      { this._summary = v; return this; }
    setSummaryOn(v)    { this._summaryOn = v; return this; }
    setSummaryOff(v)   { this._summaryOff = v; return this; }
    setDefaultValue(v) { this._default = v; return this; }
    setOnPreferenceChangeListener(l) { return this; }

    setVisible(v) {
        this._visible = v;
        return this;
    }

    _toManifest() {
        return {
            key:     this._key     ?? "",
            label:   this._title   ?? this._key ?? "",
            type:    "boolean",
            default: this._default ?? false,
        };
    }
};

globalThis.ListPreference = class ListPreference {
    constructor(context) {}
    setKey(v)          { this._key     = v; return this; }
    setTitle(v)        { this._title   = v; return this; }
    setSummary(v)      { this._summary = v; return this; }
    setEntries(v)      { this._entries = v; return this; }
    setEntryValues(v)  { this._values  = v; return this; }
    setDefaultValue(v) { this._default = v; return this; }
    setOnPreferenceChangeListener(l) { return this; }
    _toManifest() {
        const entries = this._entries ?? [];
        const values  = this._values  ?? entries;
        return {
            key:     this._key     ?? "",
            label:   this._title   ?? this._key ?? "",
            type:    "select",
            default: this._default ?? (values[0] ?? ""),
            options: entries.map((label, i) => ({ label, value: values[i] ?? label })),
        };
    }
};

globalThis.MultiSelectListPreference = class MultiSelectListPreference {
    constructor(context) {}
    setKey(v)          { this._key     = v; return this; }
    setTitle(v)        { this._title   = v; return this; }
    setSummary(v)      { this._summary = v; return this; }
    setEntries(v)      { this._entries = v; return this; }
    setEntryValues(v)  { this._values  = v; return this; }
    setDefaultValue(v) { this._default = v; return this; }
    setOnPreferenceChangeListener(l) { return this; }
    _toManifest() {
        const entries = this._entries ?? [];
        const values  = this._values  ?? entries;
        const defaultVal = Array.isArray(this._default)
            ? (this._default[0] ?? values[0] ?? "")
            : (this._default    ?? values[0] ?? "");
        return {
            key:     this._key     ?? "",
            label:   this._title   ?? this._key ?? "",
            type:    "select",
            default: defaultVal,
            options: entries.map((label, i) => ({ label, value: values[i] ?? label })),
        };
    }
};

globalThis.EditTextPreference = class EditTextPreference {
    constructor(context) {}

    setKey(v)          { this._key = v; return this; }
    setTitle(v)        { this._title = v; return this; }
    setSummary(v)      { this._summary = v; return this; }
    setDefaultValue(v) { this._default = v; return this; }
    setDialogTitle(v) { this._dialogTitle = v; return this; }
    setDialogMessage(v) { this._dialogMessage = v; return this; }

    setOnPreferenceChangeListener(l) {
        this._changeListener = l;
        return this;
    }

    setOnBindEditTextListener(l) {
        this._bindListener = l;
        return this;
    }

    _toManifest() {
        return {
            key: this._key ?? "",
            label: this._title ?? this._key ?? "",
            type: "string",
            default: this._default ?? "",
        };
    }
};

// PreferenceScreen / PreferenceGroup
globalThis.PreferenceScreen = class PreferenceScreen {
    constructor() {
        this._prefs = [];
    }

    getContext() {
        return {};
    }

    addPreference(p) {
        this._prefs.push(p);
    }

    getPreferences() {
        return this._prefs;
    }
};

globalThis.Application = class Application {
    constructor() {
        this._prefs = new Map();
    }

    getSharedPreferences(name, mode) {
        let pref = this._prefs.get(name);
        if (!pref) {
            pref = new SharedPreferences(name);
            this._prefs.set(name, pref);
        }
        return pref;
    }
};

globalThis.SharedPreferences = class SharedPreferences {
    constructor(name) {
        this.name = name;
    }

    _get(key, def) {
        const v = __settings?.[key];
        return v === undefined ? def : v;
    }

    getString(key, def) {
        return this._get(key, def);
    }

    getBoolean(key, def) {
        return this._get(key, def);
    }

    getInt(key, def) {
        return this._get(key, def);
    }

    edit() {
        return new SharedPreferencesEditor(this);
    }
};

globalThis.SharedPreferencesEditor = class SharedPreferencesEditor {
    constructor(prefs) {
        this.prefs = prefs;
    }

    putString() { return this; }
    putBoolean() { return this; }
    putInt() { return this; }

    apply() {}
    commit() { return true; }
};


globalThis.CheckBoxPreference = class CheckBoxPreference {
    constructor(context = null) {
        this.context = context;
        this._changeListener = null;
        this.key = "";
        this.title = "";
        this.summary = "";
        this.summaryOn = "";
        this.summaryOff = "";
        this.defaultValue = false;
        this.checked = false;
    }

    setOnPreferenceChangeListener(listener) {
        this._changeListener = listener;
        return this;
    }

    getOnPreferenceChangeListener() {
        return this._changeListener;
    }

    setKey(v) { this.key = v; }
    getKey() { return this.key; }

    setTitle(v) { this.title = v; }
    getTitle() { return this.title; }

    setSummary(v) { this.summary = v; }
    getSummary() { return this.summary; }

    setSummaryOn(v) { this.summaryOn = v; }
    setSummaryOff(v) { this.summaryOff = v; }

    setDefaultValue(v) { this.defaultValue = !!v; }
    getDefaultValue() { return this.defaultValue; }

    setChecked(v) { this.checked = !!v; }
    isChecked() { return this.checked ? 1 : 0; }
};

(function() {
    const _token = { _ctor: null };

    function _makePrefsTypeToken() {
        // Lazily resolve Application so definition order doesn't matter.
        if (!_token._ctor) _token._ctor = globalThis.Application ?? null;
        return _token;
    }

    const _PrefsInlined = function PreferencesKt_getPreferences__inlined_get_1() {
        this._token = _makePrefsTypeToken();
    };
    _PrefsInlined.prototype.getType  = function() { return _makePrefsTypeToken(); };
    _PrefsInlined.prototype.invoke   = function() { return _makePrefsTypeToken(); };
    _PrefsInlined.prototype.toString = function() { return "PreferencesKt_get_1"; };

    globalThis["PreferencesKt_getPreferences__inlined_get_1"] = _PrefsInlined;
    globalThis["PreferencesKt_get_1"] = _PrefsInlined;
    globalThis.PreferencesKt = globalThis.PreferencesKt ?? {
        getPreferences(context, name) {
            const app = globalThis.__appInstance ||= new Application();
            return app.getSharedPreferences(name ?? "prefs", 0);
        },
    };
})();