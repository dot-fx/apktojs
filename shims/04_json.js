globalThis.JsonTransformingSerializer = class JsonTransformingSerializer {
    constructor(tSerializer) {
        this.tSerializer = tSerializer;
    }

    transformDeserialize(element) {
        return element;
    }

    transformSerialize(element) {
        return element;
    }

    deserialize(decoder) {
        const transformed = this.transformDeserialize(decoder._json);
        const newDecoder = new JsonDecoder(transformed, decoder._descriptor);
        return this.tSerializer.deserialize(newDecoder);
    }

    serialize(encoder, value) {
        const transformed = this.transformSerialize(value);
        return this.tSerializer.serialize(encoder, transformed);
    }
};

globalThis.JsonPrimitive = class JsonPrimitive {
    constructor(value) { this._value = value; }
    toString()     { return String(this._value); }
    toJsonString() { return JSON.stringify(this._value); }
};

// 2. Then collections
globalThis.JsonArray = class JsonArray {
    constructor(arr) { this._arr = arr ?? []; }
    get(i)         { return this._arr[i] ?? 0; }
    [Symbol.iterator]() { return this._arr[Symbol.iterator](); }
};

globalThis.JsonObject = class JsonObject {
    constructor(map) { this._map = map ?? {}; }
    get(key)       { return this._map[key] ?? 0; }
};

function deepSerialize(value) {
    if (value == null) return null;

    if (value instanceof JsonObject || (value && typeof value === 'object' && '_map' in value)) {
        const out = {};
        const mapSource = value._map ?? value;
        for (const [k, v] of Object.entries(mapSource)) {
            if (k === '_map') continue; // Don't serialize the property name itself
            out[k] = deepSerialize(v);
        }
        return out;
    }

    if (value instanceof JsonArray || (value && typeof value === 'object' && '_arr' in value)) {
        const arrSource = value._arr ?? value;
        return arrSource.map(deepSerialize);
    }

    if (value instanceof JsonPrimitive) {
        return value._value;
    }

    if (Array.isArray(value)) {
        return value.map(deepSerialize);
    }

    if (typeof value === 'object') {
        const out = {};
        for (const [k, v] of Object.entries(value)) {
            out[k] = deepSerialize(v);
        }
        return out;
    }

    return value;
}

// 4. NOW add the methods that use deepSerialize
Object.assign(JsonArray.prototype, {
    toString()     { return JSON.stringify(deepSerialize(this)); },
    toJsonString() { return JSON.stringify(deepSerialize(this)); },
});

Object.assign(JsonObject.prototype, {
    toString()     { return JSON.stringify(deepSerialize(this)); },
    toJsonString() { return JSON.stringify(deepSerialize(this)); },
    toJsonRequestBody() {
        return {
            contentType: "application/json",
            content: JSON.stringify(deepSerialize(this)),
        };
    },
});

JsonObject.Companion = {
    serializer() {
        return {
            descriptor: { serialName: "JsonObject" },
            serialize: (encoder, value) => { /* ... */ },
            deserialize: (decoder) => { return new JsonObject(decoder._json); }
        };
    }
};

// 5. Builders
globalThis.JsonArrayBuilder = class JsonArrayBuilder {
    constructor() { this._arr = []; }
    add(value) { this._arr.push(value); }
    build() { return new JsonArray(this._arr); }
};

globalThis.JsonObjectBuilder = class JsonObjectBuilder {
    constructor() { this._map = {}; }
    put(key, value) {
        this._map[key] = value instanceof JsonPrimitive ? value._value : value;
    }
    build() { return new JsonObject(this._map); }
};

function callBlock(block, arg) {
    if (typeof block === 'function') {
        block(arg);
    } else {
        block.invoke(arg);
    }
}

globalThis.JsonElementBuildersKt = {
    put(builder, key, value) {
        if (builder instanceof JsonObjectBuilder) {
            builder.put(key, value);
        }
    },
    buildJsonObject(block) {
        const builder = new JsonObjectBuilder();
        callBlock(block, builder);
        return builder.build();
    },
    putJsonArray(builder, key, block) {
        const arr = new JsonArrayBuilder();
        callBlock(block, arr);
        builder.put(key, arr.build());
    },
    addJsonObject(builder, block) {
        const obj = new JsonObjectBuilder();
        callBlock(block, obj);
        builder.add(obj.build());
    },
    buildJsonArray(block) {
        const arr = new JsonArrayBuilder();
        callBlock(block, arr);
        return arr.build();
    },
};

JsonArray.Companion = {
    serializer() {
        return { descriptor: { serialName: "JsonArray" } };
    }
};

globalThis.StringSerializer = {
    INSTANCE: {
        deserialize(decoder) {
            const val = decoder._json;
            if (val === null || val === undefined) return 0;
            return String(val);
        },
        serialize(encoder, value) { return String(value ?? ""); },
        getDescriptor() { return { _fields: [], serialName: "kotlin.String" }; },
    }
};

// IntSerializer, BooleanSerializer etc. while we're at it
globalThis.IntSerializer = {
    INSTANCE: {
        deserialize(decoder) { return decoder._json ?? 0 | 0; },
        getDescriptor() { return { _fields: [], serialName: "kotlin.Int" }; },
    }
};
globalThis.BooleanSerializer = {
    INSTANCE: {
        deserialize(decoder) { return decoder._json ? 1 : 0; },
        getDescriptor() { return { _fields: [], serialName: "kotlin.Boolean" }; },
    }
};
globalThis.LongSerializer = {
    INSTANCE: {
        deserialize(decoder) { return decoder._json ?? 0; },
        getDescriptor() { return { _fields: [], serialName: "kotlin.Long" }; },
    }
};


globalThis.JsonDecoder = class JsonDecoder {
    constructor(json, descriptor) {
        this._json = json;
        this._descriptor = descriptor;
        this._index = 0;
    }

    decodeNullableSerializableElement(descriptor, index, serializer, old) {
        const key = descriptor._fields[index];
        const val = this._json[key];
        if (val === null || val === undefined) return 0;
        if (serializer && typeof serializer.deserialize === 'function') {
            const childDescriptor = serializer.getDescriptor?.() ?? descriptor;
            return serializer.deserialize(new JsonDecoder(val, childDescriptor));
        }
        return val;
    }

    decodeLongElement(descriptor, index) {
        let value;

        if (Array.isArray(this._json)) {
            value = this._json[index];
        } else {
            value = this._json[descriptor.getElementName(index)];
        }

        return value;
    }

    beginStructure(descriptor) {
        const json = this._json instanceof JsonArray ? this._json._arr : this._json;
        return new JsonDecoder(json, descriptor);
    }
    endStructure(descriptor) {}
    decodeSequentially() {
        return 0;
    }
    decodeElementIndex(descriptor) {
        const idx =
            this._index < descriptor._fields.length
                ? this._index++
                : -1;

        return idx;
    }
    decodeStringElement(descriptor, index) {
        const key = descriptor._fields[index];
        return this._json[key] ?? 0;
    }
    decodeIntElement(descriptor, index) {
        const key = descriptor._fields[index];
        return this._json[key] ?? 0;
    }
    decodeBooleanElement(descriptor, index) {
        const key = descriptor._fields[index];
        return this._json[key] ? 1 : 0;
    }
    decodeSerializableElement(descriptor, index, serializer, old) {
        const key = descriptor._fields[index];
        const val = this._json[key];
        if (val === undefined || val === null) return old ?? 0;
        if (serializer && typeof serializer.deserialize === 'function') {
            const childDescriptor = serializer.getDescriptor?.() ?? descriptor;
            return serializer.deserialize(new JsonDecoder(val, childDescriptor));
        }
        return val;
    }
};

globalThis.Json = {
    Default: {
        decodeFromString(deserializer, str) {
            const parsed = JSON.parse(str);
            const data = parsed?.data ?? parsed;
            const decoder = new JsonDecoder(data, null);
            return deserializer.deserialize(decoder);
        },
        encodeToString(serializer, obj) {
            return obj?.toJsonString
                ? obj.toJsonString()
                : JSON.stringify(deepSerialize(obj));
        }
    },
    decodeFromString(deserializer, str) {
        const parsed = JSON.parse(str);
        const data = parsed?.data ?? parsed;
        const decoder = new JsonDecoder(data, null);
        return deserializer.deserialize(decoder);
    },
    encodeToString(serializer, obj) {
        return obj?.toJsonString
            ? obj.toJsonString()
            : JSON.stringify(deepSerialize(obj));
    }
};


globalThis.OkioStreamsKt = {
    decodeFromBufferedSource(deserializer, type, source) {
        try {
            const text = typeof source === "string" ? source : source?._text ?? "";
            const parsed = JSON.parse(text);
            const actualSerializer = type ?? deserializer;
            const decoder = new JsonDecoder(parsed, null);
            return actualSerializer.deserialize(decoder);
        } catch(e) {
            console.log("error msg:", e.message);
            throw e;
        }
    },
};

globalThis.SerializersKt = {
    serializer(klass) { return klass?.Companion ?? klass; },
};
globalThis.BuiltinSerializersKt = {
    ListSerializer(elementSerializer) {
        return new ArrayListSerializer(elementSerializer);
    },
    ArrayListSerializer(elementSerializer) {
        return new ArrayListSerializer(elementSerializer);
    },
    NullableSerializer(elementSerializer) {
        return {
            _elem: elementSerializer,
            deserialize(decoder) {
                const val = decoder._json;
                if (val === null || val === undefined) return 0;
                return elementSerializer.deserialize(new JsonDecoder(val, decoder._descriptor));
            },
            getDescriptor() { return elementSerializer.getDescriptor?.() ?? null; },
        };
    },
};