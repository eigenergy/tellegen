"""Build MCP argument models from the checked-in Rust JSON Schema."""

from __future__ import annotations

import json
import operator
from pathlib import Path
from typing import Annotated, Any, Dict, List, Literal, Union, cast

from pydantic import ConfigDict, Field, create_model

_SCHEMA = json.loads(Path(__file__).with_name("_engine_schema.json").read_text())
_MODELS: Dict[str, Any] = {}
_KEYWORDS = {
    "$ref", "type", "const", "enum", "oneOf", "anyOf", "properties",
    "additionalProperties", "required", "items", "default", "description",
    "title", "format", "minimum", "exclusiveMinimum", "maximum",
    "exclusiveMaximum", "minItems", "maxItems", "minLength", "maxLength", "pattern",
}


def _type(node: Any, name: str) -> Any:
    if node is False or not isinstance(node, (dict, bool)):
        raise ValueError(f"Unsupported Rust schema in {name}: {node}")
    if isinstance(node, dict):
        unsupported = set(node) - _KEYWORDS
        if unsupported:
            raise ValueError(f"Unsupported Rust schema keywords in {name}: {sorted(unsupported)}")
        if node.get("format") not in {None, "double", "float", "uint", "uint32", "uint64", "int32", "int64"}:
            raise ValueError(f"Unsupported Rust schema format in {name}: {node['format']}")
        if "$ref" in node and not node["$ref"].startswith("#/$defs/"):
            raise ValueError(f"Unsupported Rust schema reference in {name}: {node['$ref']}")
    base = _base_type(node, name)
    if not isinstance(node, dict):
        return base
    limits = {
        target: node[source]
        for source, target in (
            ("minimum", "ge"),
            ("exclusiveMinimum", "gt"),
            ("maximum", "le"),
            ("exclusiveMaximum", "lt"),
            ("minItems", "min_length"),
            ("maxItems", "max_length"),
            ("minLength", "min_length"),
            ("maxLength", "max_length"),
            ("pattern", "pattern"),
        )
        if source in node
    }
    return Annotated[base, Field(**limits)] if limits else base


def _base_type(node: Any, name: str) -> Any:
    if node is True or node == {}:
        return Any
    if "$ref" in node:
        return model(node["$ref"].removeprefix("#/$defs/"))
    if "const" in node:
        return Literal[node["const"]]
    if "enum" in node:
        return Literal[tuple(node["enum"])]
    alternatives = node.get("oneOf", node.get("anyOf"))
    if alternatives:
        types = tuple(
            _type(value, f"{name}{index}") for index, value in enumerate(alternatives)
        )
        return types[0] if len(types) == 1 else Union[types]
    kind = node.get("type")
    if isinstance(kind, list):
        return Union[tuple(_type({**node, "type": part}, name) for part in kind)]
    if kind == "object":
        if "properties" not in node:
            return operator.getitem(
                cast(Any, Dict),
                (str, _type(node.get("additionalProperties", True), name + "Value")),
            )
        fields = {}
        for field, value in node["properties"].items():
            annotation = _type(value, name + field.title())
            default = value.get(
                "default", ... if field in node.get("required", []) else None
            )
            fields[field] = (
                annotation,
                Field(default=default, description=value.get("description")),
            )
        result = create_model(
            name,
            __config__=ConfigDict(extra="forbid", allow_inf_nan=False),
            **cast(Any, fields),
        )
        result.__doc__ = node.get("description", "Generated from Rust.")
        return result
    if kind == "array":
        return operator.getitem(
            cast(Any, List), _type(node.get("items", True), name + "Item")
        )
    if kind in {"string", "number", "integer", "boolean", "null"}:
        return {
            "string": str,
            "number": float,
            "integer": int,
            "boolean": bool,
            "null": type(None),
        }[kind]
    raise ValueError(f"Unsupported Rust schema in {name}: {node}")


def model(name: str) -> Any:
    if name not in _MODELS:
        _MODELS[name] = _type(_SCHEMA["$defs"][name], name)
    return _MODELS[name]
