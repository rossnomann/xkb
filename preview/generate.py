#!/usr/bin/env python
import enum
import sys
from collections import defaultdict
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).parent
SYMBOLS_ROOT = HERE.parent / "xkb" / "symbols"
TPL_ROOT = HERE
TPL_ORTHOLINEAR = TPL_ROOT / "ortholinear.svg"
TPL_STAGGERED = TPL_ROOT / "staggered.svg"
OUT_ROOT = HERE

LABEL_MAP = {
    "iso_level3_shift": "RALT", "aacute": "Á", "ampersand": "&amp;", "apostrophe": "'",
    "asciicircum": "^", "asciitilde": "~", "asterisk": "*", "at": "@", "backslash": "\\",
    "bar": "|", "braceleft": "{", "braceright": "}", "bracketleft": "[", "bracketright": "]",
    "ccaron": "Č", "colon": ":", "comma": ",", "cyrillic_a": "А", "cyrillic_be": "Б",
    "cyrillic_che": "Ч", "cyrillic_de": "Д", "cyrillic_e": "Э", "cyrillic_ef": "Ф",
    "cyrillic_el": "Л", "cyrillic_em": "М", "cyrillic_en": "Н", "cyrillic_er": "Р",
    "cyrillic_es": "С", "cyrillic_ghe": "Г", "cyrillic_ha": "Х", "cyrillic_hardsign": "Ъ",
    "cyrillic_i": "И", "cyrillic_ie": "Е", "cyrillic_io": "Ё", "cyrillic_ka": "К",
    "cyrillic_o": "О", "cyrillic_pe": "П", "cyrillic_sha": "Ш", "cyrillic_shcha": "Щ",
    "cyrillic_shorti": "Й", "cyrillic_softsign": "Ь", "cyrillic_te": "Т",
    "cyrillic_tse": "Ц", "cyrillic_u": "У", "cyrillic_ve": "В", "cyrillic_ya": "Я",
    "cyrillic_yeru": "Ы", "cyrillic_yu": "Ю", "cyrillic_ze": "З", "cyrillic_zhe": "Ж",
    "dcaron": "Ď", "degree": "°", "dollar": "$", "eacute": "É", "emdash": "—",
    "equal": "=", "eurosign": "€", "exclam": "!", "grave": "`", "greater": "&gt;",
    "guillemetleft": "«", "guillemetright": "»", "hyphen": "‐", "iacute": "Í",
    "lacute": "Ĺ", "lcaron": "Ľ", "less": "&lt;", "minus": "-", "ncaron": "Ň",
    "nobreakspace": "NBSPC", "nosymbol": "", "numbersign": "#", "numerosign": "№",
    "oacute": "Ó",
    "ocircumflex": "Ô", "parenleft": "(", "parenright": ")", "percent": "%",
    "period": ".", "plus": "+", "question": "?", "quotedbl": "\"", "racute": "Ŕ",
    "scaron": "Š", "semicolon": ";", "slash": "/", "space": "SPACE", "tcaron": "Ť",
    "u20bd": "₽", "uacute": "Ú", "underscore": "_", "yacute": "Ý", "zcaron": "Ž",
}


def format_label(data: str) -> str:
    key = data.strip(" ").strip(",").lower()
    return LABEL_MAP.get(key, key)


@dataclass
class Key:
    code: str
    labels: list[str]


type KeyMap = dict[str, Key]


class State(enum.StrEnum):
    initial = enum.auto()
    key_start = enum.auto()
    code = enum.auto()
    body_start = enum.auto()
    symbol_data = enum.auto()
    body_end = enum.auto()


class KeyParser:
    def __init__(self) -> None:
        self._state = State.initial
        self._key: Key | None = None

    def _get_key(self) -> Key:
        if self._key is None:
            msg = "Key is not initialized"
            raise RuntimeError(msg)
        return self._key

    def _check_key_set(self) -> None:
        if self._key is None:
            msg = "Key is not initialized"
            raise RuntimeError(msg)

    def _check_key_unset(self) -> None:
        if self._key is not None:
            msg = "Previous key is not finished"
            raise RuntimeError(msg)

    def feed(self, data: str) -> KeyMap:
        result = {}
        match self._state:
            case State.initial:
                if data.startswith("key"):
                    self._check_key_unset()
                    self._state = State.key_start
            case State.key_start:
                if data.startswith("<") and data.endswith(">"):
                    self._state = State.code
                    self._check_key_unset()
                    self._key = Key(code=data, labels=[])
                else:
                    msg = f"Invalid key code {data}"
                    raise RuntimeError(msg)
            case State.code:
                self._check_key_set()
                if data == "{":
                    self._state = State.body_start
                else:
                    msg = f"Expects body start, got {data}"
                    raise RuntimeError(msg)
            case State.body_start:
                self._check_key_set()
                if data == "[":
                    self._state = State.symbol_data
                else:
                    msg = f"Expects seq start, got {data}"
                    raise RuntimeError(msg)
            case State.symbol_data:
                if data == "]" or data == "],":
                    self._state = State.body_end
                else:
                    key = self._get_key()
                    key.labels.append(format_label(data))
            case State.body_end:
                if data == "};":
                    key = self._get_key()
                    result[key.code] = key
                    self._key = None
                    self._state = State.initial
                else:
                    self._check_key_set()
        return result


def parse_keys(data: str) -> Iterable[Key]:
    keys: dict[str, Key] = {}
    key_parser = KeyParser()
    for raw_line in data.split("\n"):
        parts = raw_line.strip().split(" ")
        for raw_part in parts:
            part = raw_part.strip(" ")
            if part:
                keys.update(key_parser.feed(part))
    return keys.values()


class RenderError(Exception):
    pass


def render_keys(template: str, keys: Iterable[Key]) -> str:
    cx = {}
    for key in keys:
        code = key.code[1:-1]
        labels = key.labels[:]
        for _i in range(4 - len(labels)):
            labels.append("")
        for a, b in [[0, 1], [2, 3]]:
            if labels[a] == labels[b]:
                labels[b] = ""
                labels[a] = labels[a].upper()
        for idx, label in enumerate(labels, start=1):
            cx[f"{code}{idx}"] = label
    try:
        return template.format(**cx)
    except KeyError as exc:
        msg = f"{exc} is not found"
        raise RenderError(msg) from None


def main() -> None:
    templates = [
        ["staggered", TPL_STAGGERED.read_text()],
        ["ortholinear", TPL_ORTHOLINEAR.read_text()]
    ]
    for sym_path in SYMBOLS_ROOT.iterdir():
        if not sym_path.is_file():
            continue
        sym_entry = sym_path.stem
        print(f"Found {sym_entry}")
        sym_data = sym_path.read_text()
        keys = parse_keys(sym_data)
        for tpl_variant, tpl_data in templates:
            out_name = f"{sym_entry}_{tpl_variant}.svg"
            try:
                out_data = render_keys(tpl_data, keys)
            except RenderError as exc:
                sys.stderr.write(f"Rendering of {out_name} has failed: {exc}\n")
            else:
                (OUT_ROOT / out_name).write_text(out_data)
                print(f"Written {out_name}")


if __name__ == "__main__":
    main()
