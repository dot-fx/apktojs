globalThis.Character = {
    isLetterOrDigit(ch) {
        if (typeof ch === "number") {
            ch = String.fromCharCode(ch);
        }
        return /[a-zA-Z0-9]/.test(ch) ? 1 : 0;
    },
    isLetter(ch) {
        if (typeof ch === "number") ch = String.fromCharCode(ch);
        return /[a-zA-Z]/.test(ch) ? 1 : 0;
    },
    isDigit(ch) {
        if (typeof ch === "number") ch = String.fromCharCode(ch);
        return /[0-9]/.test(ch) ? 1 : 0;
    },
    isWhitespace(ch) {
        if (typeof ch === "number") ch = String.fromCharCode(ch);
        return /\s/.test(ch) ? 1 : 0;
    },
    isUpperCase(ch) {
        if (typeof ch === "number") ch = String.fromCharCode(ch);
        return ch === ch.toUpperCase() && /[a-zA-Z]/.test(ch) ? 1 : 0;
    },
    isLowerCase(ch) {
        if (typeof ch === "number") ch = String.fromCharCode(ch);
        return ch === ch.toLowerCase() && /[a-zA-Z]/.test(ch) ? 1 : 0;
    },
    toUpperCase(ch) {
        if (typeof ch === "number") return String.fromCharCode(ch).toUpperCase().charCodeAt(0);
        return ch.toUpperCase();
    },
    toLowerCase(ch) {
        if (typeof ch === "number") return String.fromCharCode(ch).toLowerCase().charCodeAt(0);
        return ch.toLowerCase();
    },
    toString(ch) {
        if (typeof ch === "number") return String.fromCharCode(ch);
        return String(ch);
    },
};

globalThis.Charsets = {
    UTF_8: "UTF-8",
    UTF_16: "UTF-16",
    US_ASCII: "US-ASCII",
    ISO_8859_1: "ISO-8859-1",
};

globalThis.IntRange = class IntRange {
    constructor(start, endInclusive) {
        this.first = start;
        this.last = endInclusive;
    }

    *[Symbol.iterator]() {
        for (let i = this.first; i <= this.last; i++) {
            yield i;
        }
    }
};

globalThis.StringsKt = {

    split$default(str, delimiters, ignoreCase, limit, mask, marker) {
        if (str == null) return _makeKotlinList([]);
        if ((mask & 2) !== 0) ignoreCase = false;
        if ((mask & 4) !== 0) limit = 0;
        const seps = Array.isArray(delimiters) ? delimiters : [delimiters];
        const lim = (limit && limit > 0) ? limit : Infinity;
        const escaped = seps.map(s => String(s).replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
        const re = new RegExp(escaped.join('|'), ignoreCase ? 'i' : '');
        const result = [];
        let rest = str;
        while (result.length < lim - 1) {
            const m = rest.match(re);
            if (!m) break;
            result.push(rest.slice(0, m.index));
            rest = rest.slice(m.index + m[0].length);
        }
        result.push(rest);
        return _makeKotlinList(result);
    },

    split(str, delimiters, ignoreCase, limit) {
        return this.split$default(str, delimiters, ignoreCase, limit, 0, null);
    },

    substringBeforeLast$default(str, delimiter, missingDelimiterValue, mask, marker) {
        if (str == null) return str;

        if ((mask & 2) !== 0) {
            missingDelimiterValue = str;
        }

        const idx = String(str).lastIndexOf(String(delimiter));

        if (idx < 0) {
            return missingDelimiterValue;
        }

        return String(str).substring(0, idx);
    },

    substringBeforeLast(str, delimiter, missingDelimiterValue) {
        if (str == null) return str;
        const idx = String(str).lastIndexOf(String(delimiter));
        if (idx < 0) return missingDelimiterValue !== undefined ? missingDelimiterValue : str;
        return String(str).substring(0, idx);
    },

    trimStart(str) {
        if (str == null) return str;
        return String(str).replace(/^\s+/, "");
    },

    trimEnd(str) {
        if (str == null) return str;
        return String(str).replace(/\s+$/, "");
    },

    startsWith(str, prefix, startIndex, ignoreCase) {
        if (str == null) return 0;
        startIndex = startIndex || 0;
        if (ignoreCase) {
            return str.slice(startIndex).toLowerCase().startsWith(String(prefix).toLowerCase()) ? 1 : 0;
        }
        return str.startsWith(prefix, startIndex) ? 1 : 0;
    },

    "substringAfterLast$default"(str, delimiter, missingDelimiterValue, mask, marker) {
        if (str == null) return str;
        if ((mask & 2) !== 0) missingDelimiterValue = str;
        const idx = str.lastIndexOf(delimiter);
        return idx === -1 ? missingDelimiterValue : str.slice(idx + delimiter.length);
    },
    substringAfterLast(str, delimiter, missingDelimiterValue) {
        if (str == null) return str;
        if (missingDelimiterValue === undefined) missingDelimiterValue = str;
        const idx = str.lastIndexOf(delimiter);
        return idx === -1 ? missingDelimiterValue : str.slice(idx + delimiter.length);
    },

    startsWith$default(str, prefix, ignoreCase, mask, marker) {
        if (str == null) return 0;
        // ignoreCase is the only optional param (index 1) -> bit 1<<1 = 2,
        // not 4 (mirrors substringBefore$default / replace$default pattern).
        if ((mask & 2) !== 0) {
            ignoreCase = 0;
        }

        const isCaseIgnored = !!ignoreCase;

        if (isCaseIgnored) {
            return str.toLowerCase().startsWith(String(prefix).toLowerCase()) ? 1 : 0;
        } else {
            return str.startsWith(prefix) ? 1 : 0;
        }
    },

    endsWith(str, suffix, ignoreCase = false) {
        if (str == null) return 0;
        if (ignoreCase) {
            return str.toLowerCase().endsWith(String(suffix).toLowerCase()) ? 1 : 0;
        }
        return str.endsWith(suffix) ? 1 : 0;
    },

    contains$default(str, other, ignoreCase, mask, marker) {
        if (str == null) return 0;
        if ((mask & 2) !== 0) ignoreCase = false;
        if (ignoreCase) {
            return str.toLowerCase().includes(String(other).toLowerCase()) ? 1 : 0;
        }
        return str.includes(String(other)) ? 1 : 0;
    },

    replaceFirst$default(str, oldValue, newValue, ignoreCase, mask, marker) {
        if (str == null) return str;
        if ((mask & 4) !== 0) ignoreCase = false;
        if (ignoreCase) {
            const escapedOld = String(oldValue).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
            return str.replace(new RegExp(escapedOld, 'i'), newValue);
        }
        return str.replace(oldValue, newValue);
    },

    replaceFirst(str, oldValue, newValue, ignoreCase) {
        return this.replaceFirst$default(str, oldValue, newValue, ignoreCase, 0, null);
    },

    isBlank(str) {
        return (str == null || typeof str !== "string" || str.trim().length === 0) ? 1 : 0;
    },

    isNotBlank(str) {
        return this.isBlank(str) ? 0 : 1;
    },

    isEmpty(str) {
        return (str == null || str.length === 0) ? 1 : 0;
    },

    isNotEmpty(str) {
        return (str != null && str.length > 0) ? 1 : 0;
    },

    toIntOrNull(str, radix) {
        if (str == null || typeof str !== "string") return null;
        radix = radix || 10;
        const s = str.trim();
        if (s === "" || s === "+" || s === "-") return null;

        const body = (s[0] === '+' || s[0] === '-') ? s.slice(1) : s;
        if (body === "") return null;

        const digits = "0123456789abcdefghijklmnopqrstuvwxyz".slice(0, radix);
        for (const ch of body.toLowerCase()) {
            if (!digits.includes(ch)) return null;
        }

        const n = parseInt(s, radix);
        if (Number.isNaN(n)) return null;

        return {
            _value: n,
            intValue() { return n; },
            longValue() { return n; },
            floatValue() { return n; },
            doubleValue() { return n; },
            toString() { return String(n); },
            valueOf() { return n; },
        };
    },

    toDoubleOrNull(str) {
        if (str == null || typeof str !== "string") return null;
        const s = str.trim();
        if (s === "" || Number.isNaN(Number(s))) return null;
        const n = Number(s);
        return {
            _value: n,
            intValue() { return n | 0; },
            longValue() { return n; },
            floatValue() { return n; },
            doubleValue() { return n; },
            toString() { return String(n); },
            valueOf() { return n; },
        };
    },

    removeSuffix(str, suffix) {
        if (str == null) return str;
        return str.endsWith(suffix) ? str.slice(0, -suffix.length) : str;
    },

    removePrefix(str, prefix) {
        if (str == null) return str;
        return str.startsWith(prefix) ? str.slice(prefix.length) : str;
    },

    substringBefore$default(str, delimiter, missingDelimiterValue, mask, marker) {
        if (str == null) return str;
        if (mask & 2) {
            missingDelimiterValue = str;
        }

        const idx = str.indexOf(delimiter);

        return idx === -1
            ? missingDelimiterValue
            : str.slice(0, idx);
    },

    substringBefore(str, delimiter, missingDelimiterValue) {
        if (str == null) return str;
        const idx = str.indexOf(delimiter);
        return idx === -1
            ? (missingDelimiterValue !== undefined ? missingDelimiterValue : str)
            : str.slice(0, idx);
    },

    endsWith$default(str, suffix, ignoreCase, mask, marker) {
        if (str == null) return 0;
        if ((mask & 2) !== 0) ignoreCase = false;
        let result = false;
        if (ignoreCase) {
            result = str.toLowerCase().endsWith(String(suffix).toLowerCase());
        } else {
            result = str.endsWith(suffix);
        }
        return result ? 1 : 0; // Return Dalvik boolean format
    },

    replace$default(str, oldValue, newValue, ignoreCase, mask, marker) {
        if (str == null) return str;

        str = String(str);
        oldValue = String(oldValue);
        newValue = String(newValue);

        if ((mask & 4) !== 0) {
            ignoreCase = false;
        }

        if (ignoreCase) {
            const escapedOld = oldValue.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
            return str.replace(new RegExp(escapedOld, 'gi'), newValue);
        }

        return str.split(oldValue).join(newValue);
    },

    replace(str, oldValue, newValue, ignoreCase) {
        return this.replace$default(str, oldValue, newValue, ignoreCase, 0, null);
    },

    trim(str) {
        return (typeof str === "string") ? str.trim() : str;
    },

    append(sb, parts) {
        if (Array.isArray(parts)) {
            for (const p of parts) sb.append(p);
        } else {
            sb.append(parts);
        }
        return sb;
    },

    contains(str, other, ignoreCase) {
        if (str == null) return 0;
        ignoreCase = ignoreCase ? true : false;
        if (ignoreCase) {
            return str.toLowerCase().includes(String(other).toLowerCase()) ? 1 : 0;
        }
        return str.includes(String(other)) ? 1 : 0;
    },

    substringAfter$default(str, delimiter, missingDelimiterValue, mask, marker) {
        if (str == null) return str;

        if ((mask & 2) !== 0) {
            missingDelimiterValue = str;
        }

        const idx = str.indexOf(delimiter);

        return idx === -1
            ? missingDelimiterValue
            : str.slice(idx + delimiter.length);
    },

    substringAfter(str, delimiter, missingDelimiterValue) {
        if (str == null) return str;
        const idx = str.indexOf(delimiter);
        return idx === -1
            ? (missingDelimiterValue ?? str)
            : str.slice(idx + delimiter.length);
    },

    indexOf$default(str, other, startIndex, ignoreCase, mask, marker) {
        if (str == null) return -1;
        if ((mask & 2) !== 0) startIndex = 0;
        if ((mask & 4) !== 0) ignoreCase = false;
        if (ignoreCase) {
            return str.toLowerCase().indexOf(String(other).toLowerCase(), startIndex);
        }
        return str.indexOf(other, startIndex);
    },

    indexOf(str, other, startIndex, ignoreCase) {
        return this.indexOf$default(str, other, startIndex, ignoreCase, 0, null);
    },

    lastIndexOf$default(str, other, startIndex, ignoreCase, mask, marker) {
        if (str == null) return -1;
        if ((mask & 2) !== 0) startIndex = str.length;
        if ((mask & 4) !== 0) ignoreCase = false;
        if (ignoreCase) {
            return str.toLowerCase().lastIndexOf(String(other).toLowerCase(), startIndex);
        }
        return str.lastIndexOf(other, startIndex);
    },

    lastIndexOf(str, other, startIndex, ignoreCase) {
        const si = startIndex === undefined ? str.length : startIndex;
        return this.lastIndexOf$default(str, other, si, ignoreCase, 0, null);
    },
};

globalThis.ArrayDeque = class ArrayDeque {
    constructor(initial) {
        this._data = [];

        if (initial != null) {
            if (Array.isArray(initial)) {
                this._data.push(...initial);
            } else if (typeof initial[Symbol.iterator] === "function") {
                this._data.push(...initial);
            }
        }
    }

    add(v)            { this._data.push(v); return true; }
    addLast(v)        { this._data.push(v); }
    addFirst(v)       { this._data.unshift(v); }

    offer(v)          { this._data.push(v); return true; }
    offerLast(v)      { return this.offer(v); }
    offerFirst(v)     { this._data.unshift(v); return true; }

    push(v)           { this._data.unshift(v); }
    pop()             { return this._data.shift(); }

    remove()          { return this._data.shift(); }
    removeFirst()     { return this._data.shift(); }
    removeLast()      { return this._data.pop(); }

    poll()            { return this._data.shift() ?? null; }
    pollFirst()       { return this.poll(); }
    pollLast() {
        return this._data.length ? this._data.pop() : null;
    }

    getFirst()        { return this._data[0]; }
    getLast()         { return this._data[this._data.length - 1]; }

    peek()            { return this._data[0] ?? null; }
    peekFirst()       { return this.peek(); }
    peekLast() {
        return this._data.length
            ? this._data[this._data.length - 1]
            : null;
    }

    clear()           { this._data.length = 0; }
    size()            { return this._data.length; }
    isEmpty()         { return this._data.length === 0; }

    contains(v)       { return this._data.includes(v); }

    iterator() {
        let i = 0;
        const arr = this._data;
        return {
            hasNext() { return i < arr.length; },
            next()    { return arr[i++]; }
        };
    }

    [Symbol.iterator]() {
        return this._data[Symbol.iterator]();
    }

    toArray() {
        return this._data.slice();
    }
};

globalThis.CollectionsKt = {
    build(builder) {
        return builder;
    },
    reversed(collection) {
        const arr = [..._unwrapCollection(collection)];
        return _makeKotlinList(arr.reverse());
    },
    joinToString$default(collection, separator, prefix, postfix, limit, truncated, transform, flags, marker) {
        if (!collection) return "";
        if (!flags || (flags & 1))  separator = ", ";
        if (!flags || (flags & 2))  prefix    = "";
        if (!flags || (flags & 4))  postfix   = "";
        if (!flags || (flags & 8))  limit     = -1;
        if (!flags || (flags & 16)) truncated = "...";
        if (!flags || (flags & 32)) transform = null;

        let items = _unwrapCollection(collection);
        if (typeof _wrapKotlinObject === 'function') {
            items = items.map(_wrapKotlinObject);
        }

        let over = false;
        if (limit >= 0 && items.length > limit) { items = items.slice(0, limit); over = true; }
        const parts = items.map(x => {
            return transform ? transform(x) : String(x);
        });
        if (over) parts.push(truncated);
        return prefix + parts.join(separator) + postfix;
    },
    flatten(collections) {
        const out = [];

        for (const collection of _unwrapCollection(collections)) {
            if (collection == null) continue;

            for (const item of _unwrapCollection(collection)) {
                out.push(item);
            }
        }

        return _makeKotlinList(out);
    },
    last(collection) {
        const arr = _unwrapCollection(collection);
        if (arr.length === 0) {
            throw new Error("NoSuchElementException");
        }
        return arr[arr.length - 1];
    },
    throwIndexOverflow() { throw new RangeError("Index overflow"); },
    listOf(...args) { return args.length === 1 && Array.isArray(args[0]) ? args[0] : Array.from(args); },
    toList(collection) { return _unwrapCollection(collection); },
    listOfNotNull(arr) {
        if (Array.isArray(arr)) {
            return arr.filter(x => x !== null && x !== undefined && x !== 0);
        }
        const unwrapped = _unwrapCollection(arr);
        if (unwrapped.length > 0) return unwrapped.filter(x => x !== null && x !== undefined && x !== 0);
        return arr != null && arr !== 0 ? [arr] : [];
    },
    collectionSizeOrDefault(collection, default_) {
        if (!collection) return 0;
        if (collection.length !== undefined) return collection.length;
        if (collection.size !== undefined) return typeof collection.size === 'function' ? collection.size() : collection.size;
        return _unwrapCollection(collection).length ?? default_;
    },
    addAll(collection, elements) {
        const target = collection?._a ?? collection?._arr ?? collection;
        const items = _unwrapCollection(elements);
        for (const item of items) {
            if (typeof target.push === 'function') {
                target.push(item);
            } else if (typeof target.add === 'function') {
                target.add(item);
            }
        }
        return true;
    },
    randomOrNull(collection, random) {
        const arr = _unwrapCollection(collection);
        if (arr.length === 0) return 0;
        return arr[Math.floor(Math.random() * arr.length)] ?? 0;
    },

    drop(collection, n) {
        const arr = _unwrapCollection(collection);
        return _makeKotlinList(arr.slice(Math.max(0, n)));
    },

    take(collection, n) {
        return _unwrapCollection(collection).slice(0, Math.max(0, n));
    },

    dropLast(collection, n) {
        const arr = _unwrapCollection(collection);
        return arr.slice(0, Math.max(0, arr.length - n));
    },

    takeLast(collection, n) {
        const arr = _unwrapCollection(collection);
        return arr.slice(Math.max(0, arr.length - n));
    },

    emptyMap:     () => new LinkedHashMap(),
    emptySet:     () => new Set(),
    setOf:        (...args) => new Set(args),
    mapOf:        (...args) => new LinkedHashMap(args),
    plus:         (a, b)   => [..._unwrapCollection(a), ..._unwrapCollection(b)],
    single:       (list)   => { const arr = _unwrapCollection(list); if (arr.length !== 1) throw new Error("Expected single element"); return arr[0]; },
    firstOrNull:  (list, pred) => {
        const arr = _unwrapCollection(list);
        return pred ? (arr.find(pred) ?? null) : (arr[0] ?? null);
    },
    filter:       (list, pred) => _unwrapCollection(list).filter(pred),
    map:          (list, fn)   => _unwrapCollection(list).map(fn),
    forEach:      (list, fn)   => _unwrapCollection(list).forEach(fn),

    emptyList:     () => _mutableList(),
    toMutableList: (col) => _mutableList(_unwrapCollection(col)),
    mutableListOf: (...args) => _mutableList(args.length === 1 && Array.isArray(args[0]) ? args[0] : args),
    createListBuilder: () => _mutableList(),
    getOrNull(collection, index) {
        const arr = _unwrapCollection(collection);
        return (index >= 0 && index < arr.length) ? arr[index] : null;
    },

    first(collection, predicate) {
        const arr = _unwrapCollection(collection);
        if (predicate) {
            const found = arr.find(predicate);
            if (found === undefined) throw new Error("NoSuchElementException");
            return found;
        }
        if (arr.length === 0) throw new Error("NoSuchElementException");
        return arr[0];
    },
};

globalThis.Pair = class Pair {
    constructor(first, second) {
        this.first = first;
        this.second = second;
    }

    getFirst() {
        return this.first;
    }

    getSecond() {
        return this.second;
    }

    component1() {
        return this.first;
    }

    component2() {
        return this.second;
    }

    toString() {
        return `(${this.first}, ${this.second})`;
    }
};

globalThis.LinkedHashMap = class LinkedHashMap extends Map {
    constructor(init) {
        super();

        // Kotlin LinkedHashMap(capacity)
        if (typeof init === "number" || init == null) {
            return;
        }

        // LinkedHashMap(existingMap)
        if (init instanceof Map) {
            for (const [k, v] of init.entries()) {
                this.set(k, v);
            }
            return;
        }

        // Iterable<Entry<K,V>>
        if (Symbol.iterator in Object(init)) {
            for (const [k, v] of init) {
                this.set(k, v);
            }
        }
    }

    get(key) { return super.get(key) ?? null; }
    put(key, value) { this.set(key, value); return null; }
    containsKey(key) { return this.has(key) ? 1 : 0; }
    containsValue(val) {
        for (const v of super.values()) {
            if (v === val) return 1;
        }
        return 0;
    }
    remove(key) {
        const v = this.get(key);
        this.delete(key);
        return v;
    }
    isEmpty() { return this.size === 0 ? 1 : 0; }
    size() { return super.size; }
    entrySet() {
        const entries = [...super.entries()].map(([k, v]) => ({
            getKey: () => k,
            getValue: () => v,
        }));

        entries.iterator = function() {
            let i = 0;
            return {
                hasNext: () => (i < entries.length ? 1 : 0),
                next: () => entries[i++],
            };
        };

        return entries;
    }
    keySet() {
        return [...super.keys()];
    }

    values() {
        return [...super.values()];
    }

    putAll(map) {
        if (map instanceof Map) {
            for (const [k, v] of map) {
                this.set(k, v);
            }
        }
        return this;
    }
};

globalThis.HashMap = LinkedHashMap;

globalThis.StringBuilder = class StringBuilder {
    constructor(initial = "") { this._s = (initial == null) ? "" : "" + initial; }
    toString() { return this._s; }
    get length() { return this._s.length; }
};

StringBuilder.prototype.append = function(v, start, end) {
    const s = (v == null) ? "" : "" + v;
    this._s += (start !== undefined) ? s.slice(start, end) : s;
    return this;
};

globalThis.ArraysKt = {
    plus(arr, element) {
        const a = Array.isArray(arr) ? arr : Array.from(arr ?? []);
        return Array.isArray(element) ? [...a, ...element] : [...a, element];
    },
};

globalThis.ArrayList = class ArrayList {
    constructor() { this._a = []; }
    push(item) {
        if (item !== undefined) this._a.push(item);
        return this;
    }
    add(item)    { this._a.push(item); return true; }
    get(i)       { return this._a[i]; }
    toArray()    { return [...this._a]; }
    [Symbol.iterator]() { return this._a[Symbol.iterator](); }
    map(fn) { return this._a.map(fn); }

    iterator() {
        let i = 0; const a = this._a;
        return {
            hasNext() { return i < a.length ? 1 : 0; },
            next() { return a[i++]; }
        };
    }

    isEmpty() { return this._a.length === 0 ? 1 : 0; }  // 1=true, 0=false
    size()    { return this._a.length; }

    forEach(cb)        { this._a.forEach(cb); }
    filter(cb)         { return this._a.filter(cb); }

    contains(item) {
        return this._a.includes(item) ? 1 : 0;
    }
    [Symbol.iterator](){ return this._a[Symbol.iterator](); }

    get length() { return this._a.length; }
    get length_val() { return this._a.length; }
};

globalThis.HashSet = class HashSet {
    constructor(iterable) {
        this._s = new Set();
        if (iterable) {
            for (const item of iterable) {
                this._s.add(item);
            }
        }
    }

    push(item) {
        this._s.add(item);
        return this;
    }

    add(item) {
        this._s.add(item);
        return this;
    }

    remove(item) {
        return this._s.delete(item) ? 1 : 0;
    }

    contains(item) {
        return this._s.has(item) ? 1 : 0;
    }

    size() {
        return this._s.size;
    }

    isEmpty() {
        return this._s.size === 0 ? 1 : 0;
    }

    clear() {
        this._s.clear();
    }

    toArray() {
        return Array.from(this._s);
    }

    forEach(fn) {
        this._s.forEach(fn);
    }

    [Symbol.iterator]() {
        return this._s[Symbol.iterator]();
    }

    get length_val() {
        return this._s.size;
    }

    get size_val() {
        return this._s.size;
    }

    addAll(items) {
        for (const item of items) {
            this._s.add(item);
        }
        return this;
    }

    removeAll(items) {
        for (const item of items) {
            this._s.delete(item);
        }
        return this;
    }

    containsAll(items) {
        for (const item of items) {
            if (!this._s.has(item)) return 0;
        }
        return 1;
    }

    toList() {
        return Array.from(this._s);
    }
};

globalThis.ArrayListSerializer = class ArrayListSerializer {
    constructor(elementSerializer) {
        this._elementSerializer = elementSerializer;
    }
    deserialize(decoder) {
        let raw = decoder._json;

        if (raw?.__isJsonArray) raw = raw._arr;
        const arr = Array.isArray(raw) ? raw : [];
        const list = new ArrayList();
        for (const item of arr) {
            if (this._elementSerializer && typeof this._elementSerializer.deserialize === "function") {
                const childDescriptor = this._elementSerializer.getDescriptor?.() ?? decoder._descriptor;
                list.add(this._elementSerializer.deserialize(new JsonDecoder(item, childDescriptor)));
            } else {
                list.add(_wrapKotlinObject(item));
            }
        }
        return list;
    }
    getDescriptor() { return null; }
};

globalThis.LinkedHashSet = class LinkedHashSet extends Set {
    constructor(init) {
        super();

        if (init == null || typeof init === "number") {
            return;
        }

        if (Symbol.iterator in Object(init)) {
            for (const v of init) {
                this.add(v);
            }
        }
    }

    add(value) {
        super.add(value);
        return this;
    }

    contains(value) {
        return this.has(value) ? 1 : 0;
    }

    remove(value) {
        const existed = this.has(value);
        this.delete(value);
        return existed ? 1 : 0;
    }

    isEmpty() {
        return this.size === 0 ? 1 : 0;
    }

    size() {
        return super.size;
    }

    iterator() {
        const arr = [...this];
        let i = 0;

        return {
            hasNext: () => (i < arr.length ? 1 : 0),
            next: () => arr[i++],
        };
    }

    addAll(collection) {
        for (const v of collection) {
            this.add(v);
        }
        return this;
    }

    clear() {
        super.clear();
    }

    toArray() {
        return [...this];
    }
};

globalThis.HashSet = globalThis.LinkedHashSet;

const _COROUTINE_SUSPENDED = Symbol("COROUTINE_SUSPENDED");

globalThis.IntrinsicsKt = {
    getCOROUTINE_SUSPENDED() { return _COROUTINE_SUSPENDED; },
};

globalThis.Lambda = class Lambda {
    constructor(arity) {
        this.arity = arity ?? 0;
    }

    // invoke() is overridden by every concrete lambda subclass;
    // the base just forwards up to 4 positional args
    invoke(p0, p1, p2, p3) {
        return undefined;
    }

    toString() {
        return `Lambda/${this.arity}`;
    }
};

// Kotlin also emits FunctionN base types; alias them all to Lambda
globalThis.Function0  = Lambda;
globalThis.Function1  = Lambda;
globalThis.Function2  = Lambda;
globalThis.Function3  = Lambda;
globalThis.Function4  = Lambda;
globalThis.FunctionN  = Lambda;

globalThis.SetsKt = {
    emptySet() {
        return new Set();
    },

    contains(set, value) {
        if (set == null) return 0;
        // Works for native Set and any array-like
        if (set instanceof Set) return set.has(value) ? 1 : 0;
        if (Array.isArray(set)) return set.includes(value) ? 1 : 0;
        return 0;
    },

    hashSetOf(...items) {
        // Kotlin hashSetOf(vararg)
        if (items.length === 1 && Array.isArray(items[0])) {
            return new Set(items[0]);
        }
        return new Set(items);
    },

    setOf(...items) {
        return new Set(items);
    },

    mutableSetOf(...items) {
        return new Set(items);
    },

    plus(set, value) {
        const s = new Set(set);

        if (value instanceof Set) {
            for (const v of value) s.add(v);
        } else {
            s.add(value);
        }

        return s;
    },

    minus(set, value) {
        const s = new Set(set);

        if (value instanceof Set) {
            for (const v of value) s.delete(v);
        } else {
            s.delete(value);
        }

        return s;
    },
};

globalThis.TuplesKt = {
    to(first, second) {
        return {
            first,
            second,
            getFirst()  { return first; },
            getSecond() { return second; },
            component1() { return first; },
            component2() { return second; },
        };
    }
};

globalThis.Long = {
    valueOf(value, ignored) {
        const n = typeof value === "number" ? value : parseInt(value) || 0;
        return {
            _value: n,
            longValue() { return n; },
            intValue() { return n; },
            floatValue() { return n; },
            doubleValue() { return n; },
            toString() { return String(n); },
            valueOf() { return n; },
            selectFirst() { return 0; },  // guard against register reuse
            text() { return ""; },
            attr() { return ""; },
        };
    }
};

globalThis.MapsKt = {
    mapCapacity(expectedSize) {
        expectedSize = Number(expectedSize) || 0;

        if (expectedSize < 3) {
            return expectedSize + 1;
        }

        if (expectedSize < 1073741824) {
            return expectedSize + Math.floor(expectedSize / 3);
        }

        return 2147483647;
    },

    mutableMapOf(...args) {
        const map = new Map();
        if (args.length === 1 && Array.isArray(args[0])) {
            for (const pair of args[0]) {
                if (pair && pair.first !== undefined) {
                    map.set(pair.first, pair.second);
                } else if (Array.isArray(pair)) {
                    map.set(pair[0], pair[1]);
                }
            }
        }
        return map;
    },

    withDefault(map, defaultFn) {
        return new Proxy(map, {
            get(target, prop, receiver) {
                if (prop in target) {
                    return Reflect.get(target, prop, receiver);
                }

                if (typeof prop === "string") {
                    return defaultFn(prop);
                }

                return undefined;
            }
        });
    },

    mapOf(...pairs) {
        const map = new LinkedHashMap();
        for (const pair of pairs) {
            if (pair == null) continue;
            if (Array.isArray(pair)) {
                map.put(pair[0], pair[1]);
            } else if (pair.first !== undefined && pair.second !== undefined) {
                map.put(pair.first, pair.second);
            } else if (typeof pair.getFirst === "function") {
                map.put(pair.getFirst(), pair.getSecond());
            }
        }
        return map;
    },

    toMap(source) {
        if (source == null) {
            return new LinkedHashMap();
        }

        if (source instanceof Map) {
            return new LinkedHashMap(source);
        }

        const map = new LinkedHashMap();

        for (const entry of source) {
            if (Array.isArray(entry)) {
                map.put(entry[0], entry[1]);
            } else if (entry?.getFirst && entry?.getSecond) {
                map.put(entry.getFirst(), entry.getSecond());
            } else if (entry?.first !== undefined && entry?.second !== undefined) {
                map.put(entry.first, entry.second);
            }
        }

        return map;
    },

    toMutableMap(source) {
        return source instanceof Map
            ? new LinkedHashMap(source)
            : this.toMap(source);
    },

    toList(source) {
        if (source == null) return [];
        if (source instanceof LinkedHashMap) {
            return Array.from(source.entries()).map(([k, v]) => TuplesKt.to(k, v));
        }
        if (source instanceof Map) {
            return Array.from(source.entries()).map(([k, v]) => TuplesKt.to(k, v));
        }
        if (Array.isArray(source)) return source;
        return Array.from(source);
    },
};

globalThis.FunctionReference = class FunctionReference {};

globalThis.ContinuationImpl = class ContinuationImpl {
    constructor(completion) {
        this.completion = completion || null;
        this.label = 0;
    }

    invokeSuspend(result) {
        return result;
    }

    resumeWith(result) {
        let current = this;
        let param = result;

        while (current) {
            try {
                const outcome = current.invokeSuspend(param);

                if (outcome === COROUTINE_SUSPENDED) {
                    return outcome;
                }

                param = outcome;
            } catch (e) {
                param = e;
            }

            current = current.completion;
        }

        return param;
    }

    create(value, completion) {
        return this;
    }
};

globalThis.CoroutineImpl = globalThis.SuspendLambda;

globalThis.ResultKt = {
    throwOnFailure(result) {
        if (result && result.__isFailure) {
            throw result.cause ?? new Error("Coroutine failed");
        }
    },
};

globalThis.ReentrantLock = class ReentrantLock {
    constructor(fair = false) {
        this._fair = !!fair;
        this._holdCount = 0;
        this._owner = null;
    }

    lock() {
        const me = Symbol.for("__js_thread__");

        if (this._owner === null || this._owner === me) {
            this._owner = me;
            this._holdCount++;
            return;
        }

        throw new Error("ReentrantLock shim does not support contention");
    }

    unlock() {
        if (this._holdCount > 0) {
            this._holdCount--;

            if (this._holdCount === 0) {
                this._owner = null;
            }
        }
    }

    tryLock() {
        try {
            this.lock();
            return true;
        } catch {
            return false;
        }
    }

    isLocked() {
        return this._holdCount > 0;
    }

    getHoldCount() {
        return this._holdCount;
    }

    isFair() {
        return this._fair;
    }

    newCondition() {
        return {};
    }
};



globalThis.CharsKt = {
    isWhitespace(ch) { return /\s/.test(ch); },
};

globalThis.ordinal = function(v) { return typeof v === "number" ? v : v?.ordinal ?? 0; };

globalThis.Dispatchers = {
    getIO()      { return { type: "IO" }; },
    getMain()    { return { type: "Main" }; },
    getDefault() { return { type: "Default" }; },
};

globalThis.CoroutineScopeKt = {
    CoroutineScope(context) {
        return { _context: context };
    }
};


globalThis.kotlin = Object.assign(globalThis.kotlin ?? {}, { Unit: { INSTANCE: undefined } });
globalThis.Unit = { INSTANCE: { toString() { return "kotlin.Unit"; } } };

globalThis.java = {
    util: { Locale: { ROOT: "root" } },
};

globalThis.BuildersKt = {
    launch$default(scope, context, start, block) {
        if (typeof block === "function") {
            Promise.resolve().then(() => block());
        } else if (block?.invoke) {
            Promise.resolve().then(() => block.invoke(scope, null));
        }
    },

    runBlocking(ctx, block) {
        let result;

        if (typeof block?.invoke === "function") {
            result = block.invoke(null, null);
        } else if (typeof block?.invokeSuspend === "function") {
            result = block.invokeSuspend(Unit_INSTANCE);
        } else if (typeof block === "function") {
            result = block();
        } else {
            throw new Error("runBlocking: invalid block");
        }

        if (
            result === COROUTINE_SUSPENDED ||
            result === _COROUTINE_SUSPENDED
        ) {
            throw new Error(
                "Coroutine suspension unsupported in sync runtime"
            );
        }

        return result;
    }
};


globalThis.PropertyResourceBundle = class PropertyResourceBundle {
    constructor(reader) {
        this._data = {};
        // Can't load .properties files in JS, fallback to empty
        // Intl will return [key] for missing keys which is fine
    }

    containsKey(key) {
        return 0; // always miss, fall through to [key] fallback
    }

    getString(key) {
        return `[${key}]`;
    }
};

globalThis.InputStreamReader = class InputStreamReader {
    constructor(stream, encoding) {}
};

globalThis.PluginGeneratedSerialDescriptor = class PluginGeneratedSerialDescriptor {
    constructor(name, serializer, size) {
        this._name = name;
        this._serializer = serializer;
        this._fields = [];
        this._annotations = [];
        this._elementAnnotations = [];
    }
    addElement(name, isOptional) {
        this._fields.push(name);
    }
    getElementIndex(name) {
        return this._fields.indexOf(name);
    }
    getElementName(index) {
        return this._fields[index];
    }

    pushAnnotation(annotation) {
        this._annotations.push(annotation);
    }

    getAnnotations() {
        return this._annotations;
    }
};

globalThis.PluginExceptionsKt = {
    throwMissingFieldException(seenBits, requiresBits, descriptor) {
        throw new Error(`Missing required field in ${descriptor?.serialName ?? "unknown"}`);
    },
};

globalThis.Intrinsics = {
    areEqual(a, b) {
        if (a === b) return 1;
        if (a === null || b === null) return 0;
        if (typeof a === 'object' && typeof a.equals === 'function') {
            return a.equals(b) ? 1 : 0;
        }
        return 0;
    },

    checkNotNull(value, message) {
        if (value === null || value === undefined) {
            throw new Error(message ?? "Required value was null");
        }
        return value;
    },

    checkNotNullParameter(value, name) {
        if (value === null || value === undefined) {
            throw new Error(`Parameter specified as non-null is null: ${name}`);
        }
        return value;
    },

    checkExpressionValueIsNotNull(value, expression) {
        if (value === null || value === undefined) {
            throw new Error(`Expression '${expression}' must not be null`);
        }
        return value;
    },

    checkFieldIsNotNull(value, className, fieldName) {
        if (value === null || value === undefined) {
            throw new Error(`Field '${fieldName}' in '${className}' must not be null`);
        }
        return value;
    },

    throwUninitializedPropertyAccessException(name) {
        throw new Error(`lateinit property ${name} has not been initialized`);
    },

    throwNpe() {
        throw new Error("NullPointerException");
    },

    stringPlus(a, b) {
        return String(a ?? "null") + String(b ?? "null");
    },

    areEqualOrBothNull(a, b) {
        if (a === null && b === null) return 1;
        if (a === null || b === null) return 0;
        return a === b ? 1 : 0;
    },
};

globalThis.ContextKt_special__inlined_get_1 = class ContextKt_special__inlined_get_1 {
    getType() { return null; }
};

globalThis.KTypeProjection = {
    Companion: {
        invariant(type) { return { variance: "INVARIANT", type }; },
        covariant(type) { return { variance: "COVARIANT", type }; },
        contravariant(type) { return { variance: "CONTRAVARIANT", type }; },
        STAR: { variance: "STAR", type: null },
    },
};

globalThis.Reflection = {
    typeOf(cls, ...projections) { return { classifier: cls, arguments: projections }; },
};

globalThis.RangesKt = {
    downTo(from, to) {
        return {
            iterator() {
                let i = from;

                return {
                    hasNext() {
                        return i >= to ? true : 0;
                    },

                    nextInt() {
                        return i--;
                    },

                    next() {
                        return this.nextInt();
                    },
                };
            },

            [Symbol.iterator]() {
                let i = from;

                return {
                    next() {
                        return i >= to
                            ? { value: i--, done: false }
                            : { done: true };
                    },
                };
            },
        };
    },

    until(from, to) {
        return {
            iterator() {
                let i = from;

                return {
                    hasNext() {
                        return i < to ? true : 0;
                    },

                    nextInt() {
                        return i++;
                    },

                    next() {
                        return this.nextInt();
                    },
                };
            },

            [Symbol.iterator]() {
                let i = from;

                return {
                    next() {
                        return i < to
                            ? { value: i++, done: false }
                            : { done: true };
                    },
                };
            },
        };
    },

    step(range, step) {
        return range;
    },

    coerceAtMost(value, max) {
        return value > max ? max : value;
    },

    coerceAtLeast(value, min) {
        return value < min ? min : value;
    },

    coerceIn(value, min, max) {
        return value < min
            ? min
            : value > max
                ? max
                : value;
    },
};

globalThis.kotlin = globalThis.kotlin ?? {};
kotlin.LazyThreadSafetyMode = {
    PUBLICATION:    { name: "PUBLICATION" },
    SYNCHRONIZED:   { name: "SYNCHRONIZED" },
    NONE:           { name: "NONE" },
};

globalThis.LazyThreadSafetyMode = kotlin.LazyThreadSafetyMode;

globalThis.LazyKt = {
    lazy(modeOrInitializer, initializer) {
        const init = typeof modeOrInitializer === 'function'
            ? modeOrInitializer
            : initializer;

        let value;
        let initialized = false;
        return {
            getValue() {
                if (!initialized) {
                    value = init();
                    initialized = true;
                    console.log("lazy initialized:", typeof value, value?.constructor?.name);
                }
                return value;
            }
        };
    }
};


globalThis.java = globalThis.java ?? {};
globalThis.java.util = globalThis.java.util ?? {};
globalThis.java.util.List = { _type: "List" };

java.util.Calendar = {
    getInstance() {
        const now = new Date();
        return {
            _date: now,
            get(field) {
                switch(field) {
                    case java.util.Calendar.YEAR:         return now.getFullYear();
                    case java.util.Calendar.MONTH:        return now.getMonth();
                    case java.util.Calendar.DAY_OF_MONTH: return now.getDate();
                    case java.util.Calendar.HOUR_OF_DAY:  return now.getHours();
                    case java.util.Calendar.MINUTE:       return now.getMinutes();
                    case java.util.Calendar.SECOND:       return now.getSeconds();
                    default: return 0;
                }
            },
            getTimeInMillis() { return now.getTime(); },
            getTime()         { return now; },
        };
    },
    YEAR:         1,
    MONTH:        2,
    DAY_OF_MONTH: 5,
    HOUR_OF_DAY:  11,
    MINUTE:       12,
    SECOND:       13,
};

java.util.concurrent = {
    TimeUnit: {
        SECONDS:      { toMillis(v) { return v * 1000; },    toSeconds(v) { return v; } },
        MINUTES:      { toMillis(v) { return v * 60000; },   toSeconds(v) { return v * 60; } },
        HOURS:        { toMillis(v) { return v * 3600000; },  toSeconds(v) { return v * 3600; } },
        MILLISECONDS: { toMillis(v) { return v; },            toSeconds(v) { return Math.floor(v / 1000); } },
        DAYS:         { toMillis(v) { return v * 86400000; }, toSeconds(v) { return v * 86400; } },
    },
};

globalThis.Calendar = java.util.Calendar;
globalThis.TimeUnit = java.util.concurrent.TimeUnit;

globalThis.java = globalThis.java ?? {};
java.lang = java.lang ?? {};
java.lang.Boolean = {
    TRUE:  true,
    FALSE: false,
    valueOf(v) { return !!v; },
};
java.lang.Integer = {
    valueOf(v)    { return v | 0; },
    parseInt(v)   { return parseInt(v, 10); },
    MAX_VALUE:    2147483647,
    MIN_VALUE:    -2147483648,
};
globalThis.Integer = java.lang.Integer;

globalThis.String.valueOf = (v) => String(v);
globalThis.String.format  = (fmt, ...args) => {
    let i = 0;
    return fmt.replace(/%[sdf]/g, () => String(args[i++]));
};

globalThis.Enum = class Enum {
    constructor(name, ordinal) {
        this.name = name;
        this._ordinal = ordinal ?? 0;
    }
    ordinal()  { return this._ordinal; }
    name()     { return this.name; }
    toString() { return this.name; }
};


globalThis.HelperKt_special__inlined_get_1 = class HelperKt_special__inlined_get_1 {
    getType() { return null; }
};


globalThis.DurationUnit = {
    NANOSECONDS: 'NANOSECONDS',
    MICROSECONDS: 'MICROSECONDS',
    MILLISECONDS: 'MILLISECONDS',
    SECONDS: 'SECONDS',
    MINUTES: 'MINUTES',
    HOURS: 'HOURS',
    DAYS: 'DAYS',
};

globalThis.Duration = class Duration {
    static Companion = {
        getZERO() {
            return 0;
        },

        getINFINITE() {
            return Number.MAX_SAFE_INTEGER;
        }
    };
};
globalThis.DurationKt = {
    toDuration(value, unit) {
        return {
            value,
            unit,

            inWholeMilliseconds() {
                switch (unit) {
                    case DurationUnit.SECONDS: return value * 1000;
                    case DurationUnit.MINUTES: return value * 60_000;
                    case DurationUnit.HOURS: return value * 3_600_000;
                    case DurationUnit.DAYS: return value * 86_400_000;
                    default: return value;
                }
            },

            toString() {
                return `${value} ${unit}`;
            }
        };
    }
};


globalThis.EnumEntriesKt = {
    enumEntries(values) {
        return values;
    }
};


globalThis.CloseableKt = {
    closeFinally(closeable, cause) {
        try { closeable?.close?.(); } catch(e) {}
    },
};

const COROUTINE_SUSPENDED = Symbol("COROUTINE_SUSPENDED");
globalThis.SuspendLambda = class SuspendLambda {
    constructor(arity, completion) {
        this.arity = arity;
        this.completion = completion || null;

        this.a_val = null;
        this.b_val = 0;

        this.label = 0;
    }

    create(value, completion) {
        this.completion = completion;
        return this;
    }

    invoke(p1, p2) {
        // p1 is the value/scope, p2 is the completion
        // Only set completion, don't reconstruct — captured fields are already set
        if (p2 !== undefined && p2 !== null) {
            this.completion = p2;
        }
        return this.invokeSuspend(Unit_INSTANCE);
    }

    resumeWith(result) {
        let current = this;
        let param = result;

        while (current) {
            try {
                const outcome = current.invokeSuspend(param);

                if (outcome === COROUTINE_SUSPENDED) {
                    return COROUTINE_SUSPENDED;
                }

                param = outcome;
            } catch (e) {
                param = e;
            }

            current = current.completion;
        }

        return param;
    }

    invokeSuspend(result) {
        return Unit_INSTANCE;
    }
};

globalThis.FunctionReferenceImpl = class FunctionReferenceImpl {
    constructor(
        arity,
        owner,
        name,
        signature,
        flags
    ) {
        this.arity = arity;
        this.owner = owner;
        this.name = name;
        this.signature = signature;
        this.flags = flags;
    }
};

globalThis.FullTypeReference = class FullTypeReference {
    constructor(...args) {
        this._typeArgs = args;
        // If the first arg is a constructor, stash it so getInstance() can find it.
        this._ctor = (typeof args[0] === "function") ? args[0] : null;
    }

    getType() {
        // Return `this` so that getInstance() can inspect _ctor directly,
        // rather than returning the raw args array (which is not a constructor).
        return this;
    }

    toString() {
        return "FullTypeReference";
    }
};

const Unit_INSTANCE = { toString() { return "kotlin.Unit"; } };
globalThis.Unit_INSTANCE = Unit_INSTANCE;
if (!globalThis.kotlin) globalThis.kotlin = {};
if (!globalThis.kotlin.Unit) globalThis.kotlin.Unit = { INSTANCE: Unit_INSTANCE };

// IllegalStateException

globalThis.Exception = class Exception extends Error {
    constructor(msg) { super(msg ?? "Exception"); this.name = "Exception"; }
};

globalThis.Null = class Null extends Error {
    constructor(msg) { super(msg ?? "null"); this.name = "Null"; }
};

globalThis.NullPointerException = class NullPointerException extends Error {
    constructor(msg) { super(msg ?? "NullPointerException"); this.name = "NullPointerException"; }
};

globalThis.IllegalStateException = class IllegalStateException extends Error {
    constructor(msg) { super(msg ?? "IllegalStateException"); this.name = "IllegalStateException"; }
};

globalThis.IllegalArgumentException = class IllegalArgumentException extends Error {
    constructor(msg) { super(msg ?? "IllegalArgumentException"); this.name = "IllegalArgumentException"; }
};

globalThis.RuntimeException = class RuntimeException extends Error {
    constructor(msg) { super(msg ?? "RuntimeException"); this.name = "RuntimeException"; }
};

globalThis.UnsupportedOperationException = class UnsupportedOperationException extends Error {
    constructor(msg) { super(msg ?? "UnsupportedOperationException"); this.name = "UnsupportedOperationException"; }
};

globalThis.IndexOutOfBoundsException = class IndexOutOfBoundsException extends Error {
    constructor(msg) { super(msg ?? "IndexOutOfBoundsException"); this.name = "IndexOutOfBoundsException"; }
};

globalThis.NoSuchElementException = class NoSuchElementException extends Error {
    constructor(msg) { super(msg ?? "NoSuchElementException"); this.name = "NoSuchElementException"; }
};