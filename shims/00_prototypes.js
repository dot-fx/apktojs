if (!Number.prototype.intValue) {
    Number.prototype.intValue  = function() { return this | 0; };
    Number.prototype.longValue = function() { return this; };
}


if (!Array.prototype.iterator) {
    Array.prototype.iterator = function() {
        let i = 0;
        const arr = this;
        return {
            hasNext: () => i < arr.length ? true : 0,
            next: () => i < arr.length ? arr[i++] : 0,
        };
    };
}

if (!Array.prototype.clone) {
    Array.prototype.clone = function() { return [...this]; };
}
if (!Number.prototype.ordinal) {
    Number.prototype.ordinal = function() { return this.valueOf(); };
}
Object.defineProperty(Array.prototype, 'firstInstance', {
    value: function(predicate) {
        const result = this.find(predicate);
        return result === undefined ? null : result;
    },
    enumerable: false,
    writable: true,
    configurable: true
});
Array.prototype.get = function(i) { return this[i]; };

// Polyfill to catch obfuscated Dalvik array lengths
let lastValidUrlString = "";
Object.defineProperty(String.prototype, 'length_val', {
    get: function() {
        if (this.startsWith("http://") || this.startsWith("https://")) {
            lastValidUrlString = this.toString();
            return this.length;
        }
        if (this.toString() === "" && lastValidUrlString !== "") {
            return lastValidUrlString.length;
        }

        return this.length;
    },
    enumerable: false,
    configurable: true
});

String.prototype.compareTo = function(other) {
    if (this < other) return -1;
    if (this > other) return 1;
    return 0;
};

String.prototype.substringBefore = function(delimiter) {
    const idx = this.indexOf(delimiter);
    return idx === -1 ? this.toString() : this.slice(0, idx);
};

String.prototype.substringAfter = function(delimiter) {
    const idx = this.indexOf(delimiter);
    return idx === -1 ? "" : this.slice(idx + delimiter.length);
};

Object.defineProperty(Array.prototype, 'length_val', {
    get() { return this.length; },
    enumerable: false,
    configurable: true,
});

if (!String.prototype.hashCode) {
    String.prototype.hashCode = function() {
        let h = 0;
        for (let i = 0; i < this.length; i++) {
            h = (Math.imul(31, h) + this.charCodeAt(i)) | 0;
        }
        return h;
    };
}

if (!Number.prototype.hashCode) {
    Number.prototype.hashCode = function() { return this | 0; };
}

Object.defineProperty(Boolean.prototype, 'booleanValue', {
    value: function() {
        return this.valueOf() ? 1 : 0;
    },
    enumerable: false,
    writable: true,
    configurable: true
});

// Polyfill for Number just in case the transpiler has already unboxed it into an int
Object.defineProperty(Number.prototype, 'booleanValue', {
    value: function() {
        return this.valueOf() !== 0 ? 1 : 0;
    },
    enumerable: false,
    writable: true,
    configurable: true
});

Object.defineProperty(Boolean, "valueOf", {
    value(v) {
        return !!v;
    },
    writable: true,
    configurable: true
});

if (!Array.prototype.toArray) {
    Array.prototype.toArray = function(target) { return [...this]; };
}

Set.prototype.contains = function(value) {
    return this.has(value) ? 1 : 0;
};

Array.prototype.contains = function(value) {
    return this.includes(value) ? 1 : 0;
};

String.prototype.contains = function(value) {
    return this.includes(value) ? 1 : 0;
};

Function.prototype.invoke = function(...args) {
    return this(...args);
};

String.prototype.getBytes = function(charset) {
    const str = this;
    const bytes = [];
    for (let i = 0; i < str.length; i++) {
        const code = str.charCodeAt(i);
        if (code < 128) {
            bytes.push(code);
        } else if (code < 2048) {
            bytes.push((code >> 6) | 192);
            bytes.push((code & 63) | 128);
        } else {
            bytes.push((code >> 12) | 224);
            bytes.push(((code >> 6) & 63) | 128);
            bytes.push((code & 63) | 128);
        }
    }
    return bytes;
};

String.prototype.equals = function(other) {
    return other != null && this.valueOf() === other.valueOf();
};

String.prototype.hashCode = function() {
    let h = 0;
    for (let i = 0; i < this.length; i++) {
        h = (Math.imul(31, h) + this.charCodeAt(i)) | 0;
    }
    return h;
};

Map.prototype.put = function(k, v) { this.set(k, v); return null; };
Map.prototype.containsKey = function(k) { return this.has(k); };
Map.prototype.remove = function(k) { const v = this.get(k); this.delete(k); return v ?? null; };
Map.prototype.getOrDefault = function(k, def) { return this.has(k) ? this.get(k) : def; };