"""Migrate JSON fixture files from old MIR schema to new schema.

Transforms:
- MirTy: Adt -> Struct/Enum, FnDef -> Fn, Int/Uint -> unified Int, etc.
- MirConstValue: Scalar -> Bits, Unsupported "unevaluated:" -> Unevaluated
- Function metadata: string-typed params/return -> MirTy objects
"""

import json
import re
import sys

# Known enum types (by substring match on path)
KNOWN_ENUMS = [
    "Option", "Result", "ControlFlow", "IrqResult", "Ordering",
    "SerialErrors",
]


def is_known_enum(path: str) -> bool:
    for e in KNOWN_ENUMS:
        if e in path:
            return True
    return False


def convert_mir_ty(obj):
    """Recursively convert a MirTy JSON object from old to new schema."""
    if not isinstance(obj, dict) or "kind" not in obj:
        return obj

    kind = obj["kind"]

    if kind == "Adt":
        path = obj.get("path", "")
        old_args = obj.get("args", [])
        # Convert args recursively and preserve as synthetic fields
        converted_args = [convert_mir_ty(arg) for arg in old_args]

        if is_known_enum(path):
            # For enums with args, embed as synthetic variant fields.
            # Use Rust-standard variant ordering.
            variants = []
            if "Option" in path and converted_args:
                # Option<T>: None (variant 0), Some(T) (variant 1)
                variants = [
                    {"name": "None", "fields": []},
                    {"name": "Some", "fields": [
                        {"index": 0, "name": "0", "ty": converted_args[0]}
                    ]},
                ]
            elif "Result" in path and len(converted_args) >= 2:
                # Result<T, E>: Ok(T) (variant 0), Err(E) (variant 1)
                variants = [
                    {"name": "Ok", "fields": [
                        {"index": 0, "name": "0", "ty": converted_args[0]}
                    ]},
                    {"name": "Err", "fields": [
                        {"index": 0, "name": "0", "ty": converted_args[1]}
                    ]},
                ]
            elif converted_args:
                # Generic enum with args - embed all as first variant
                variants = [
                    {"name": "_0", "fields": [
                        {"index": i, "name": f"_{i}", "ty": arg}
                        for i, arg in enumerate(converted_args)
                    ]},
                ]
            return {"kind": "Enum", "path": path, "variants": variants}
        else:
            # Preserve type args as synthetic fields so register resolution works
            fields = [
                {"index": i, "name": f"_{i}", "ty": arg}
                for i, arg in enumerate(converted_args)
            ]
            return {"kind": "Struct", "path": path, "fields": fields}

    elif kind == "FnDef":
        path = obj.get("path", "")
        return {
            "kind": "Fn",
            "path": path,
            "params": [],
            "ret": {"kind": "Tuple", "elements": []},
        }

    elif kind == "Int":
        int_ty = obj.get("int_ty", "I32")
        return {"kind": "Int", "signed": True, "width": int_ty}

    elif kind == "Uint":
        uint_ty = obj.get("uint_ty", "U32")
        return {"kind": "Int", "signed": False, "width": uint_ty}

    elif kind == "Float":
        float_ty = obj.get("float_ty", "F32")
        return {"kind": "Float", "width": float_ty}

    elif kind == "Ref":
        inner = convert_mir_ty(obj.get("ty", {}))
        return {"kind": "Ref", "mutbl": obj.get("mutbl", False), "ty": inner}

    elif kind == "RawPtr":
        inner = convert_mir_ty(obj.get("ty", {}))
        return {"kind": "RawPtr", "mutbl": obj.get("mutbl", False), "ty": inner}

    elif kind == "Tuple":
        elements = [convert_mir_ty(e) for e in obj.get("elements", [])]
        return {"kind": "Tuple", "elements": elements}

    elif kind == "Array":
        inner = convert_mir_ty(obj.get("ty", {}))
        return {"kind": "Array", "element": inner, "len": obj.get("len", 0)}

    elif kind == "Slice":
        inner = convert_mir_ty(obj.get("ty", {}))
        return {"kind": "Slice", "element": inner}

    elif kind == "Param":
        name = obj.get("name", "?")
        return {"kind": "Unsupported", "description": f"param:{name}"}

    elif kind in ("Bool", "Char", "Str", "Never"):
        return obj

    elif kind == "Unsupported":
        return obj

    return obj


def convert_mir_const_value(obj):
    """Convert a MirConstValue JSON object from old to new schema."""
    if not isinstance(obj, dict) or "kind" not in obj:
        return obj

    kind = obj["kind"]

    if kind == "Scalar":
        return {"kind": "Bits", "value": obj.get("bits", 0)}

    elif kind == "ZeroSized":
        return obj

    elif kind == "Unsupported":
        desc = obj.get("description", "")
        if desc.startswith("unevaluated:"):
            remainder = desc[len("unevaluated:"):]
            # Keep as Unevaluated with the path (may include bracket notation).
            return {"kind": "Unevaluated", "path": remainder}
        else:
            return {"kind": "Unsupported", "path": None, "description": desc}

    return obj


def string_to_mir_ty(s: str):
    """Convert a string type representation to a MirTy object."""
    if s is None:
        return None

    s = s.strip()

    if s == "bool":
        return {"kind": "Bool"}
    if s == "char":
        return {"kind": "Char"}
    if s == "str":
        return {"kind": "Str"}
    if s == "!":
        return {"kind": "Never"}
    if s == "()":
        return {"kind": "Tuple", "elements": []}

    # Integer types
    int_map = {
        "u8": ("U8", False), "u16": ("U16", False), "u32": ("U32", False),
        "u64": ("U64", False), "u128": ("U128", False), "usize": ("Usize", False),
        "i8": ("I8", True), "i16": ("I16", True), "i32": ("I32", True),
        "i64": ("I64", True), "i128": ("I128", True), "isize": ("Isize", True),
    }
    if s in int_map:
        width, signed = int_map[s]
        return {"kind": "Int", "signed": signed, "width": width}

    # Float types
    if s == "f32":
        return {"kind": "Float", "width": "F32"}
    if s == "f64":
        return {"kind": "Float", "width": "F64"}

    # Reference types
    if s.startswith("&mut "):
        inner = string_to_mir_ty(s[5:])
        return {"kind": "Ref", "mutbl": True, "ty": inner}
    if s.startswith("&"):
        inner = string_to_mir_ty(s[1:])
        return {"kind": "Ref", "mutbl": False, "ty": inner}

    # Known enum-like names
    if any(e in s for e in KNOWN_ENUMS):
        return {"kind": "Enum", "path": s, "variants": []}

    # Default: wrap as Unsupported
    return {"kind": "Unsupported", "description": s}


def walk_and_convert(obj):
    """Recursively walk a JSON structure and convert MIR types/constants."""
    if isinstance(obj, list):
        return [walk_and_convert(item) for item in obj]

    if not isinstance(obj, dict):
        return obj

    # Check if this is a MirTy object in OLD format that needs conversion.
    # Detect old format by checking for old-specific fields.
    kind = obj.get("kind")

    is_old_mir_ty = (
        (kind == "Adt" and "variant_index" not in obj and "args" in obj)
        or (kind == "FnDef")
        or (kind == "Int" and "int_ty" in obj)
        or (kind == "Uint")
        or (kind == "Float" and "float_ty" in obj)
        or (kind == "Param" and "name" in obj and "path" not in obj)
    )
    if is_old_mir_ty:
        result = convert_mir_ty(obj)
        return walk_and_convert(result)

    # MirConstValue variants
    if kind == "Scalar":
        return convert_mir_const_value(obj)

    if kind == "Unsupported" and "description" in obj:
        desc = obj["description"]
        if isinstance(desc, str) and desc.startswith("unevaluated:"):
            return convert_mir_const_value(obj)
        # Add path field if missing (for MirConstValue::Unsupported)
        if "path" not in obj and "ty" not in obj:
            # This is likely a MirConstValue::Unsupported, not MirTy::Unsupported
            # MirTy::Unsupported doesn't have a path field
            # Distinguish by context - if parent has "value" key pointing here
            pass

    # MirTy objects that need field renames (old "ty" -> new "element")
    if kind == "Array" and "ty" in obj and "element" not in obj and "len" in obj:
        inner = walk_and_convert(convert_mir_ty(obj["ty"]))
        return {"kind": "Array", "element": inner, "len": obj.get("len", 0)}

    if kind == "Slice" and "ty" in obj and "element" not in obj and "mutbl" not in obj:
        inner = walk_and_convert(convert_mir_ty(obj["ty"]))
        return {"kind": "Slice", "element": inner}

    # Process MirConstant objects (have both "ty" and "value")
    if "ty" in obj and "value" in obj and isinstance(obj["ty"], dict):
        ty_kind = obj["ty"].get("kind")
        val_kind = obj["value"].get("kind") if isinstance(obj["value"], dict) else None

        result = {}
        for k, v in obj.items():
            if k == "ty":
                result[k] = walk_and_convert(convert_mir_ty(v))
            elif k == "value" and isinstance(v, dict):
                converted = convert_mir_const_value(v)
                result[k] = walk_and_convert(converted)
            else:
                result[k] = walk_and_convert(v)
        return result

    # Process aggregate kinds (Adt in MirAggregateKind stays as Adt)
    if kind == "Adt" and "variant_index" in obj:
        # This is MirAggregateKind::Adt, not MirTy::Adt - keep it
        result = {}
        for k, v in obj.items():
            if k == "args":
                result[k] = [walk_and_convert(convert_mir_ty(a)) for a in v]
            else:
                result[k] = walk_and_convert(v)
        return result

    # Generic recursion
    result = {}
    for k, v in obj.items():
        if k == "ty" and isinstance(v, dict) and "kind" in v:
            result[k] = walk_and_convert(convert_mir_ty(v))
        elif k == "int_ty" or k == "uint_ty" or k == "float_ty":
            # These are inside MirTy objects that should have been caught above
            result[k] = v
        else:
            result[k] = walk_and_convert(v)
    return result


def convert_function_entry(func):
    """Convert a function entry's top-level fields."""
    # Convert params[].ty from string to MirTy
    if "params" in func:
        for param in func["params"]:
            if isinstance(param.get("ty"), str):
                param["ty"] = string_to_mir_ty(param["ty"])

    # Convert return_type from string to MirTy
    if "return_type" in func and isinstance(func["return_type"], str):
        func["return_type"] = string_to_mir_ty(func["return_type"])

    # Add monomorphization field if missing
    if "monomorphization" not in func:
        func["monomorphization"] = None

    return func


def migrate_fixture(data):
    """Migrate an entire raw artifact JSON fixture."""
    # Process bodies section
    if "bodies" in data and data["bodies"]:
        functions = data["bodies"].get("functions", [])
        for func in functions:
            convert_function_entry(func)

    # Walk the entire structure to convert MIR types and constants
    return walk_and_convert(data)


def main():
    if len(sys.argv) < 2:
        print("Usage: python migrate_mir_schema.py <json_file> [--in-place]")
        sys.exit(1)

    filepath = sys.argv[1]
    in_place = "--in-place" in sys.argv

    with open(filepath) as f:
        data = json.load(f)

    migrated = migrate_fixture(data)

    if in_place:
        with open(filepath, "w") as f:
            json.dump(migrated, f, indent=2)
            f.write("\n")
        print(f"Migrated {filepath} in-place")
    else:
        print(json.dumps(migrated, indent=2))


if __name__ == "__main__":
    main()
