globalThis.SimpleDateFormat = class SimpleDateFormat {
    constructor(pattern, locale) {
        this._pattern = pattern;
        this._locale = locale;
    }

    parse(str, pos) {
        if (!str) return 0;
        const trimmed = str.trim();
        if (!trimmed) return 0;
        const patternResult = this.parseWithPattern(trimmed, this._pattern);
        if (patternResult) {
            if (pos) pos.setIndex(str.length);
            return patternResult;
        }
        const ms = Date.parse(trimmed);
        if (pos) pos.setIndex(str.length);
        return isNaN(ms) ? 0 : new Date(ms);
    }

    parseWithPattern(str, pattern) {
        const tokenMap = [
            { token: 'yyyy', re: '(\\d{4})',    key: 'year' },
            { token: 'yy',   re: '(\\d{2})',    key: 'year2' },
            { token: 'MMMM', re: '([A-Za-z]+)', key: 'monthName' },
            { token: 'MMM',  re: '([A-Za-z]+)', key: 'monthShort' },
            { token: 'MM',   re: '(\\d{1,2})',  key: 'month' },
            { token: 'M',    re: '(\\d{1,2})',  key: 'month' },
            { token: 'dd',   re: '(\\d{1,2})',  key: 'day' },
            { token: 'd',    re: '(\\d{1,2})',  key: 'day' },
            { token: 'HH',   re: '(\\d{1,2})',  key: 'hour' },
            { token: 'H',    re: '(\\d{1,2})',  key: 'hour' },
            { token: 'hh',   re: '(\\d{1,2})',  key: 'hour' },
            { token: 'h',    re: '(\\d{1,2})',  key: 'hour' },
            { token: 'mm',   re: '(\\d{1,2})',  key: 'minute' },
            { token: 'ss',   re: '(\\d{1,2})',  key: 'second' },
            { token: 'a',    re: '(AM|PM|am|pm)', key: 'ampm' },
        ];
        // longest tokens first so 'MMMM' wins over 'M'
        const sortedTokens = [...tokenMap].sort((a, b) => b.token.length - a.token.length);

        let reStr = '';
        const keys = [];
        let i = 0;

        outer:
            while (i < pattern.length) {
                // try to match a token at this position
                for (const { token, re, key } of sortedTokens) {
                    if (pattern.startsWith(token, i)) {
                        reStr += re;
                        keys.push(key);
                        i += token.length;
                        continue outer;
                    }
                }
                // no token matched: treat as a literal char, escape if needed
                const ch = pattern[i];
                reStr += /[.*+?^${}()|[\]\\]/.test(ch) ? '\\' + ch : ch;
                i++;
            }

        const match = new RegExp('^' + reStr + '$', 'i').exec(str.trim());
        if (!match) return null;

        const groups = {};
        keys.forEach((key, idx) => { groups[key] = match[idx + 1]; });

        let year   = parseInt(groups.year   ?? groups.year2 ?? '1970');
        if (groups.year2) year += year < 70 ? 2000 : 1900;
        let month  = 0;
        let day    = parseInt(groups.day ?? '1');
        let hour   = parseInt(groups.hour ?? '0');
        let minute = parseInt(groups.minute ?? '0');
        let second = parseInt(groups.second ?? '0');

        if (groups.monthName)  month = MONTHS_LONG.indexOf(groups.monthName.toLowerCase());
        else if (groups.monthShort) month = MONTHS_SHORT.indexOf(groups.monthShort.toLowerCase());
        else if (groups.month) month = parseInt(groups.month) - 1;

        if (groups.ampm) {
            const pm = groups.ampm.toLowerCase() === 'pm';
            if (pm && hour < 12) hour += 12;
            if (!pm && hour === 12) hour = 0;
        }

        return new Date(year, month, day, hour, minute, second);
    }

    format(date) {
        return (date instanceof Date ? date : new Date(date)).toISOString();
    }

    setTimeZone(tz) { this._tz = tz; }
};

globalThis.TimeZone = class TimeZone {
    constructor(id) {
        this.id = id;
    }

    getID() {
        return this.id;
    }

    static getTimeZone(id) {
        return new TimeZone(id);
    }
};

globalThis.ParsePosition = class ParsePosition {
    constructor(index) {
        this._index = index;
        this._errorIndex = -1;
    }
    getIndex()        { return this._index; }
    setIndex(v)       { this._index = v; }
    getErrorIndex()   { return this._errorIndex; }
    setErrorIndex(v)  { this._errorIndex = v; }
};

globalThis.Regex = class Regex {
    constructor(pattern, options) {
        let flags = "";

        if (options instanceof Set) {
            for (const opt of options) {
                flags += opt.flag ?? "";
            }
        } else if (Array.isArray(options)) {
            for (const opt of options) {
                flags += opt.flag ?? "";
            }
        } else if (typeof options === "string") {
            flags = options;
        }

        this._pattern = pattern;
        this._flags = flags;
        this._re = new RegExp(pattern, flags);
    }
    containsMatchIn(str)  { return this._re.test(str) ? 1 : 0; }
    matches(str)  { return new RegExp(`^(?:${this._pattern})$`).test(str) ? 1 : 0; }
    find(str, start = 0)  {
        const re = new RegExp(this._pattern, "g" + this._flags.replace("g",""));
        re.lastIndex = start;
        const m = re.exec(str);
        if (!m) return 0;
        return { value: m[0], groupValues: m, destructured: { component1: () => m[1] }, getValue() { return this; }, };
    }
    findAll(str, start = 0) {
        const re = new RegExp(this._pattern, "g" + this._flags.replace("g",""));
        re.lastIndex = start;
        const results = [];
        let m;
        while ((m = re.exec(str)) !== null) {
            results.push({ value: m[0], groupValues: m });
        }
        return results;
    }
    replace(str, replacement)    { return str.replace(this._re, replacement); }
    replaceAll(str, replacement) { return str.replaceAll(new RegExp(this._pattern, "g"), replacement); }
    split(str)                   { return str.split(this._re); }
    toString()                   { return this._pattern; }

    test(str) { return this._re.test(str) ? 1 : 0; }

    static find$default(regex, input, startIndex, flags, marker) {
        if (flags & 1) startIndex = 0;
        return regex.find(input, startIndex);
    }
};


globalThis.RegexOption = {
    IGNORE_CASE: { flag: "i" },
    MULTILINE: { flag: "m" },
    DOT_MATCHES_ALL: { flag: "s" },
    LITERAL: { flag: "" },
    COMMENTS: { flag: "" },
    UNIX_LINES: { flag: "" },
    CANON_EQ: { flag: "" },
};

RegexOption.Companion = {};

globalThis.Locale = {
    ENGLISH: "en",
    ROOT:    "root",
    US:      "en-US",
    getDefault() { return "en"; },
    forLanguageTag(tag) { return tag; },
};

globalThis.Collator = {
    getInstance(locale) {
        return {
            locale,
            compare(a, b) {
                return a.localeCompare(b, locale);
            },
        };
    },
};