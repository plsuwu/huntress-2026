#!/usr/bin/env python3
"""Query the adjacent MITRE Enterprise ATT&CK CSV, including multiline fields."""

import argparse
import csv
import json
from pathlib import Path
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("terms", nargs="*", help="Case-insensitive substrings; all must match")
    parser.add_argument("--id", action="append", default=[], help="Exact technique ID (repeatable)")
    parser.add_argument("--field", action="append", help="Search only these CSV columns (repeatable)")
    parser.add_argument("--csv", type=Path, default=Path(__file__).with_name("attackv19.2.csv"))
    parser.add_argument("--description", action="store_true", help="Print full descriptions")
    parser.add_argument("--json", action="store_true", help="Output complete matching CSV records as JSON")
    args = parser.parse_args()
    if not args.terms and not args.id:
        parser.error("supply search terms or --id")

    try:
        with args.csv.open(encoding="utf-8-sig", newline="") as stream:
            reader = csv.DictReader(stream)
            fields = args.field or ["ID", "name", "description"]
            unknown = set(fields + ["ID", "name", "description", "url"]) - set(reader.fieldnames or [])
            if unknown:
                parser.error("unknown CSV columns: " + ", ".join(sorted(unknown)))
            ids = {value.upper() for value in args.id}
            terms = [term.casefold() for term in args.terms]
            matches = []
            for row in reader:
                if ids and row["ID"].upper() not in ids:
                    continue
                text = "\n".join(row[field] or "" for field in fields).casefold()
                if all(term in text for term in terms):
                    matches.append(row)
    except (OSError, UnicodeError, csv.Error) as error:
        parser.exit(2, f"{parser.prog}: {error}\n")

    if args.json:
        print(json.dumps(matches, indent=2, ensure_ascii=False))
    else:
        for row in matches:
            print(f"{row['ID']}\t{row['name']}")
            if args.description:
                print(row["description"])
                print(row["url"])
                print()
    return 0 if matches else 1


if __name__ == "__main__":
    sys.exit(main())
