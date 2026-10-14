#!/usr/bin/env python3

"""Statically analyze and submit Huntress CTF's Host With the Most challenge.

Requires pyzipper (for example: python3 -m pip install pyzipper).
The challenge scripts are parsed as data and are never executed.

On a distribution-managed Python install, use:
    python3 -m venv .venv
    .venv/bin/pip install pyzipper
    .venv/bin/python host_with_the_most.py
"""

from __future__ import annotations

import argparse
import ast
import base64
import hashlib
import io
import json
import re
import sys
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path
from typing import Any

try:
    import pyzipper
except ImportError as exc:
    raise SystemExit("Install pyzipper first: python3 -m pip install pyzipper") from exc


PROXY = "http://10.0.0.219/huntress-ctf-proxy"
DEFAULT_CHALLENGE = "script-analysis-day-02-encoded-intent"
ARCHIVE_PASSWORD = b"infected"


def request_json(url: str, payload: dict[str, Any] | None = None) -> dict[str, Any]:
    data = None if payload is None else json.dumps(payload).encode()
    request = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json"} if data is not None else {},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.loads(response.read())


def start_attempt(challenge: str) -> dict[str, Any]:
    started = request_json(PROXY, {"challenge": challenge})
    if started.get("result") in {"already_solved", "CORRECT"}:
        query = urllib.parse.urlencode({"op": "status", "challenge": challenge})
        status = request_json(f"{PROXY}?{query}")
        return {**started, **status}
    if started.get("mode") == "mastery" and not started.get("artifacts"):
        query = urllib.parse.urlencode({"op": "status", "challenge": challenge})
        status = request_json(f"{PROXY}?{query}")
        return {**started, **status}
    return started


def retry_attempt(challenge: str) -> dict[str, Any]:
    retried = request_json(PROXY, {"op": "retry", "challenge": challenge})
    if retried.get("mode") == "mastery" and not retried.get("artifacts"):
        query = urllib.parse.urlencode({"op": "status", "challenge": challenge})
        status = request_json(f"{PROXY}?{query}")
        return {**retried, **status}
    return retried


def download_archive(challenge: str) -> bytes:
    query = urllib.parse.urlencode({"op": "artifact", "challenge": challenge})
    with urllib.request.urlopen(f"{PROXY}?{query}", timeout=30) as response:
        return response.read()


def download_capability(path: str) -> bytes:
    capability = Path(path).name
    query = urllib.parse.urlencode({"op": "download", "capability": capability})
    with urllib.request.urlopen(f"{PROXY}?{query}", timeout=30) as response:
        return response.read()


def read_archive(data: bytes) -> dict[str, str]:
    files: dict[str, str] = {}
    with pyzipper.AESZipFile(io.BytesIO(data)) as archive:
        archive.pwd = ARCHIVE_PASSWORD
        for name in archive.namelist():
            if name.endswith("/"):
                continue
            member = archive.read(name)
            if name.lower().endswith(".zip"):
                match = re.search(r"evidence-(\d+)", Path(name).stem)
                set_name = f"set{int(match.group(1))}" if match else Path(name).stem
                with pyzipper.AESZipFile(io.BytesIO(member)) as nested:
                    nested.pwd = ARCHIVE_PASSWORD
                    for nested_name in nested.namelist():
                        if nested_name.endswith("/"):
                            continue
                        files[f"{set_name}/{Path(nested_name).name}"] = nested.read(
                            nested_name
                        ).decode("utf-8-sig")
            else:
                files[Path(name).name] = member.decode("utf-8-sig")
    return files


def ps_descriptors(files: dict[str, str]) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for name, text in files.items():
        if not name.lower().endswith(".ps1"):
            continue
        match = re.search(
            r"FromBase64String\(\s*['\"]([A-Za-z0-9+/=]+)['\"]\s*\)", text
        )
        if not match:
            raise ValueError(f"Could not find static Base64 descriptor in {name}")
        result[name] = json.loads(base64.b64decode(match.group(1)))
    return result


def quoted_literals(expression: str) -> list[str]:
    return [ast.literal_eval(token) for token in re.findall(r'"(?:\\.|[^"\\])*"', expression)]


def packed_text(source: str) -> str | None:
    match = re.search(r"\bpackedText\s*=\s*(.*?);", source, re.S)
    if not match:
        return None
    literals = quoted_literals(match.group(1))
    return "".join(literals) if literals else None


def numeric_array(source: str, variable: str) -> list[int] | None:
    match = re.search(
        rf"\b{re.escape(variable)}\s*=\s*(\[[^;]*?\])\s*;", source, re.S
    )
    if not match:
        return None
    value = ast.literal_eval(match.group(1))
    if all(isinstance(item, int) for item in value):
        return value
    return None


def function_body(source: str, name: str) -> str | None:
    pattern = re.compile(
        rf"(?:\bfunction\s+{re.escape(name)}\s*\([^)]*\)|"
        rf"\b{re.escape(name)}\s*=\s*function(?:\s+[A-Za-z_$][\w$]*)?\s*\([^)]*\))\s*\{{"
    )
    match = pattern.search(source)
    if not match:
        return None
    opening = source.find("{", match.start(), match.end())
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[opening + 1 : index]
    return None


def static_decoder(source: str) -> tuple[str, str] | tuple[None, None]:
    recover_names = re.findall(r"\b(recover_[A-Za-z0-9_$]+)\s*=", source)
    recover_names += re.findall(r"\bfunction\s+(recover_[A-Za-z0-9_$]+)\s*\(", source)
    for recover_name in recover_names:
        body = function_body(source, recover_name)
        if body is None:
            continue
        decoder_names = re.findall(
            r"\bfunction\s+(decode_[A-Za-z0-9_$]+)\s*\(|"
            r"\b(decode_[A-Za-z0-9_$]+)\s*=\s*function",
            source,
        )
        declared = [a or b for a, b in decoder_names]
        called = [name for name in declared if re.search(rf"\b{re.escape(name)}\b", body)]
        if called:
            return body, function_body(source, called[-1])
        return body, None
    return "", None


def literal_payload_array(recover_body: str) -> list[int] | None:
    for match in re.finditer(
        r"\b(?:var|let)\s+[A-Za-z_$][\w$]*\s*=\s*(\[[^;]*?\])\s*;",
        recover_body,
        re.S,
    ):
        try:
            value = ast.literal_eval(match.group(1))
        except (SyntaxError, ValueError):
            continue
        if isinstance(value, list) and value and all(isinstance(item, int) for item in value):
            return value
    return None


def apply_decoder_ops(values: list[int], decoder_body: str | None) -> list[int]:
    if not decoder_body:
        return values
    key = numeric_array(decoder_body, "keyBytes")
    operations: list[tuple[int, str, Any]] = []
    reverse = decoder_body.find(".reverse()")
    if reverse >= 0:
        operations.append((reverse, "reverse", None))
    if key:
        key_op = re.search(r"\^\s*keyBytes\s*\[\s*i\s*%\s*keyBytes\.length\s*\]", decoder_body)
        if key_op:
            operations.append((key_op.start(), "xor_key", key))
    xor = re.search(r"\^\s*(\d+)", decoder_body)
    if xor:
        operations.append((xor.start(), "xor", int(xor.group(1))))
    differential = re.search(r"\[\s*i\s*\]\s*\^\s*previous\b", decoder_body)
    if differential:
        seed = re.search(r"\bvar\s+previous\s*=\s*(\d+)", decoder_body)
        operations.append((differential.start(), "xor_previous", int(seed.group(1)) if seed else 0))
    arithmetic = re.search(r"\[\s*i\s*\]\s*([+-])\s*(\d+)", decoder_body)
    if arithmetic:
        operations.append(
            (arithmetic.start(), "add" if arithmetic.group(1) == "+" else "subtract", int(arithmetic.group(2)))
        )
    for _, operation, operand in sorted(operations):
        if operation == "reverse":
            values.reverse()
        elif operation == "xor":
            values = [value ^ operand for value in values]
        elif operation == "xor_key":
            values = [value ^ operand[index % len(operand)] for index, value in enumerate(values)]
        elif operation == "xor_previous":
            original = values[:]
            previous = operand
            values = []
            for value in original:
                values.append(value ^ previous)
                previous = value
        elif operation == "add":
            values = [(value + operand) % 256 for value in values]
        elif operation == "subtract":
            values = [(value - operand) % 256 for value in values]
    return values


def decode_js_descriptor(source: str, filename: str) -> dict[str, Any]:
    """Recover the JSON descriptor using only literal data and decoder constants."""
    encoded = packed_text(source)
    plaintext: str | None = None
    values: list[int] | None = None

    if encoded is not None and encoded.startswith("%"):
        percent_bytes = [int(value, 16) for value in re.findall(r"%([0-9a-fA-F]{2})", encoded)]
        recover_body, decoder_body = static_decoder(source)
        has_byte_transform = bool(
            decoder_body
            and (
                ".reverse()" in decoder_body
                or re.search(r"\^\s*(?:\d+|previous|keyBytes)", decoder_body)
                or re.search(r"\[\s*i\s*\]\s*[+-]\s*\d+", decoder_body)
            )
        )
        if has_byte_transform or any(value >= 128 for value in percent_bytes):
            values = percent_bytes
        else:
            plaintext = urllib.parse.unquote(encoded)
    elif encoded is not None:
        if re.fullmatch(r"(?:[0-9a-fA-F]{2}:)+[0-9a-fA-F]{2}", encoded):
            values = [int(part, 16) for part in encoded.split(":")]
        elif re.fullmatch(r"(?:\d+-)+\d+", encoded):
            values = [int(part) for part in encoded.split("-")]
        else:
            values = list(base64.b64decode(encoded))
    else:
        first = numeric_array(source, "firstPart")
        second = numeric_array(source, "secondPart")
        if first is not None and second is not None:
            values = first + second
        else:
            recover_body, _ = static_decoder(source)
            values = literal_payload_array(recover_body) if recover_body else None
            if values is None:
                values = next(
                    (
                        candidate
                        for variable in ("characters", "v_a19c", "storedMaterial")
                        if (candidate := numeric_array(source, variable))
                    ),
                    None,
                )
        if values is None:
            raise ValueError(f"Unsupported static encoding in {filename}")

    recover_body, decoder_body = static_decoder(source)
    if values is not None:
        # Some samples reverse the literal byte array in recover_* before calling
        # their decoder; that operation is part of the data flow, not execution.
        if recover_body:
            call_match = re.search(r"\bdecode_[A-Za-z0-9_$]+\b", recover_body)
            reverse_match = re.search(r"\.reverse\s*\(\s*\)", recover_body)
            if reverse_match and (not call_match or reverse_match.start() < call_match.start()):
                values.reverse()
        values = apply_decoder_ops(values, decoder_body)
        plaintext = bytes(values).decode("utf-8")

    try:
        return json.loads(plaintext or "")
    except (json.JSONDecodeError, UnicodeDecodeError) as exc:
        raise ValueError(f"Decoded data in {filename} was not JSON") from exc


def js_descriptors(files: dict[str, str]) -> dict[str, dict[str, Any]]:
    return {
        name: decode_js_descriptor(text, name)
        for name, text in files.items()
        if name.lower().endswith(".js")
    }


def classify(descriptor: dict[str, Any], filename: str) -> tuple[str, str]:
    steps = descriptor.get("steps", [])
    values = descriptor.get("values", {})
    if "transform_content" in steps:
        return "impact", "T1486"
    if "upload_remote" in steps:
        return "uploader", "T1041"
    if "dispatch_memory" in steps:
        return "stager", "T1105"
    if "decode_embedded" in steps and "write_file" in steps:
        return "dropper", "T1140"
    if "handoff_script" in steps and "dispatch_process" in steps:
        return "staged_chain", "T1059.007"
    if "acquire_remote" in steps and "write_file" in steps:
        return "downloader", "T1105"
    if "dispatch_process" in steps:
        return "launcher", "T1059.007"
    if "decode_configuration" in steps and "return_configuration" in steps:
        return "benign_utility", "none"
    return "benign_utility", "none"


def inspect_function(source: str) -> str:
    match = re.search(r"\binspect_[A-Za-z0-9_$]+\s*=\s*function\b", source)
    if match:
        following = source[match.end() : match.end() + 80]
        return "named_expression" if re.match(r"\s+[A-Za-z_$]", following) else "anonymous_expression"
    if re.search(r"\bfunction\s+inspect_[A-Za-z0-9_$]+\s*\(", source):
        return "declaration"
    raise ValueError("Could not identify the inspect function form")


def inspect_body(source: str) -> str:
    match = re.search(
        r"(?:\binspect_[A-Za-z0-9_$]+\s*=\s*function(?:\s+[A-Za-z_$][\w$]*)?|\bfunction\s+inspect_[A-Za-z0-9_$]+)\s*\([^)]*\)\s*\{",
        source,
    )
    if not match:
        raise ValueError("Could not locate the inspect function body")
    opening = source.find("{", match.start(), match.end())
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[opening + 1 : index]
    raise ValueError("Inspect function body has unbalanced braces")


def invocation_style(source: str) -> str:
    match = re.search(r"\brecover_[A-Za-z0-9_$]+\s*(?:\.\s*(call|apply)\s*)?\(", source)
    if not match:
        raise ValueError("Could not identify the descriptor recovery invocation")
    return match.group(1) or "direct"


def property_style(source: str) -> str:
    styles: set[str] = set()
    descriptor = re.search(
        r"\bvar\s+([A-Za-z_$][\w$]*)\s*=\s*eval\s*\(", source
    )
    target_names = (
        [descriptor.group(1)]
        if descriptor
        else ["item4", "configuration", "v_d41a"]
    )
    pattern = re.compile(
        rf"\b(?:{'|'.join(re.escape(name) for name in target_names)})"
        r"\s*(?:\.([A-Za-z_$][\w$]*)|\[\s*['\"]([^'\"]+)['\"]\s*\])"
    )
    for match in pattern.finditer(source):
        styles.add("dot" if match.group(1) else "bracket_literal")
    if styles == {"dot"}:
        return "dot"
    if styles == {"bracket_literal"}:
        return "bracket_literal"
    if styles == {"dot", "bracket_literal"}:
        return "mixed"
    raise ValueError("Could not identify descriptor property access style")


def _build_answers_one(files: dict[str, str]) -> dict[str, str]:
    answers: dict[str, str] = {}
    ps = ps_descriptors(files)
    js = js_descriptors(files)

    for name, descriptor in ps.items():
        stem = Path(name).stem
        values = descriptor
        if stem == "resource-sample":
            answers.update(
                {
                    "resource-scenario": "resource_hijacking",
                    "resource-attack": "T1496",
                    "resource-url": values["b"],
                    "resource-wallet": values["c"],
                    "resource-mutex": values["h"],
                }
            )
        elif stem == "collection-sample":
            answers.update(
                {
                    "collection-scenario": "synthetic_collection",
                    "collection-attack": "T1555.003",
                    "collection-url": values["b"],
                    "collection-file": values["c"][0],
                    "collection-user-agent": values["f"],
                }
            )
        elif stem == "impact-sample":
            answers.update(
                {
                    "impact-scenario": "synthetic_impact",
                    "impact-attack": "T1486",
                    "impact-extension": values["b"],
                    "impact-note": values["c"],
                    "impact-url": values["d"],
                }
            )

    prefix_by_file = {
        "host-context.js": "jscript-benign",
        "memory-stage.js": "jscript-stager",
        "embedded-write.js": "jscript-dropper",
        "remote-write.js": "jscript-downloader",
        "local-dispatch.js": "jscript-launcher",
        "staged-handoff.js": "jscript-chain",
        "outbound-transfer.js": "jscript-uploader",
        "impact-plan.js": "jscript-impact",
        "relay-handoff.js": "jscript-relay",
    }
    for filename, descriptor in js.items():
        prefix = prefix_by_file.get(Path(filename).name)
        if prefix is None:
            continue
        values = descriptor.get("values", {})
        role, technique = classify(descriptor, Path(filename).name)
        body = inspect_body(files[filename])
        answers[f"{prefix}-role"] = role
        answers[f"{prefix}-attack"] = technique
        answers[f"{prefix}-adapter"] = str(descriptor.get("adapter", ""))
        if prefix != "jscript-impact":
            answers[f"{prefix}-campaign"] = str(values.get("campaign_id", ""))

        if prefix in {"jscript-benign", "jscript-launcher"}:
            indicator_key = "host_id" if prefix == "jscript-benign" else "file_path"
            indicator_suffix = "host" if prefix == "jscript-benign" else "path"
            answers[f"{prefix}-{indicator_suffix}"] = str(values[indicator_key])
        elif prefix in {"jscript-stager", "jscript-downloader"}:
            answers[f"{prefix}-url"] = str(values["url"])
        elif prefix == "jscript-dropper":
            answers[f"{prefix}-path"] = str(values["file_path"])
        elif prefix == "jscript-chain":
            answers[f"{prefix}-next-stage"] = str(values["next_stage"])
        elif prefix == "jscript-uploader":
            answers[f"{prefix}-url"] = str(values["url"])
        elif prefix == "jscript-impact":
            answers[f"{prefix}-extension"] = str(values["extension"])
            answers[f"{prefix}-note"] = str(values["note_name"])
        elif prefix == "jscript-relay":
            answers[f"{prefix}-next-stage"] = str(values["next_stage"])

        for suffix, value in (
            ("function-style", inspect_function(files[filename])),
            ("invocation-style", invocation_style(body)),
            ("property-style", property_style(body)),
        ):
            answers[f"{prefix}-{suffix}"] = value
    return answers


def build_answers(files: dict[str, str]) -> dict[str, str]:
    groups: dict[str, dict[str, str]] = {}
    for key, text in files.items():
        if "/" in key:
            group, filename = key.split("/", 1)
        else:
            group, filename = "", key
        groups.setdefault(group, {})[filename] = text
    answers: dict[str, str] = {}
    for group, members in groups.items():
        solved = _build_answers_one(members)
        if group:
            answers.update({f"{group}.{key}": value for key, value in solved.items()})
        else:
            answers.update(solved)
    return answers


def submit(challenge: str, answers: dict[str, str]) -> dict[str, Any]:
    return request_json(
        PROXY,
        {"challenge": challenge, "answers": answers},
    )


def write_run_files(
    challenge: str,
    attempt: dict[str, Any],
    archive: bytes,
    files: dict[str, str],
    answers: dict[str, str],
    result: dict[str, Any] | None,
    output_dir: Path,
    deliveries: list[tuple[dict[str, Any], bytes]] | None = None,
) -> dict[str, str]:
    output_dir.mkdir(parents=True, exist_ok=True)
    stem = challenge.replace("/", "__")
    archive_path = output_dir / f"{stem}.zip"
    start_path = output_dir / f"{stem}.start.json"
    answers_path = output_dir / f"{stem}.answers.json"
    report_path = output_dir / f"{stem}.md"
    result_path = output_dir / f"{stem}.result.json"
    archive_path.write_bytes(archive)
    delivery_paths: list[str] = []
    for index, (artifact_info, content) in enumerate(deliveries or [], 1):
        delivery_path = output_dir / f"{stem}__delivery{index}.zip"
        delivery_path.write_bytes(content)
        delivery_paths.append(str(delivery_path))
    start_path.write_text(json.dumps(attempt, indent=2, ensure_ascii=False) + "\n")
    answers_path.write_text(json.dumps(answers, indent=2, ensure_ascii=False) + "\n")
    if result is not None:
        result_path.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")

    digest = hashlib.sha256(archive).hexdigest()
    artifact_info = (attempt.get("artifacts") or [{}])[0]
    advertised_download = attempt.get("download") or artifact_info.get("download")
    advertised_name = artifact_info.get("filename") or "evidence.zip"
    mode = attempt.get("mode") or (
        challenge.rsplit("/", 1)[-1] if "/" in challenge else "single_solve"
    )
    deadline = attempt.get("deadline")
    clock_running = bool(deadline) and (result or {}).get("result") not in {
        "CORRECT",
        "already_solved",
    }
    files_by_artifact: dict[str, list[str]] = {}
    questions_by_file: dict[str, list[str]] = {}
    for question in attempt.get("questions", []):
        artifact = question.get("artifact")
        if artifact:
            filename = artifact.split(" -> ")[-1]
            files_by_artifact.setdefault(filename, []).append(question.get("id", ""))
            questions_by_file.setdefault(filename, []).append(question.get("question", ""))

    lines = [
        f"# {attempt.get('name', challenge)}",
        "",
        f"- **Challenge ID:** `{challenge}`",
        f"- **Attempt ID:** `{attempt.get('attempt_id', 'not returned')}`",
        f"- **Mode:** `{mode}`",
        f"- **Description:** {attempt.get('prompt', '')}",
        f"- **Deadline:** {deadline or 'None (no timer)'}",
        f"- **Clock running:** {'yes' if clock_running else 'no'}",
        f"- **Guidance:** {attempt.get('guidance', '')}",
        "",
        "## Assigned artifacts",
        "",
    ]
    artifact_records = attempt.get("artifacts") or []
    if artifact_records:
        for index, artifact in enumerate(artifact_records, 1):
            local_path = delivery_paths[index - 1] if index <= len(delivery_paths) else str(archive_path)
            path = artifact.get("download", advertised_download or str(archive_path))
            filename = artifact.get("filename") or advertised_name
            digest_text = artifact.get("sha256", "")
            lines.append(
                f"- **Delivery {index}:** [{filename}]({path}); SHA-256 `{digest_text}`; local copy `{local_path}`."
            )
    else:
        lines.append(
            f"- [{advertised_name}]({advertised_download or str(archive_path)}); SHA-256 `{attempt.get('sha256') or digest}`."
        )
    lines.extend(
        [
            "",
            f"- **Local bundle SHA-256:** `{digest}`.",
            "",
            "## Files and purpose",
            "",
            "The challenge groups the three PowerShell specimens with eight JScript plans. The PowerShell files provide the scenario, ATT&CK, and indicator analyses. Each JScript file reconstructs a plan descriptor; the questions below request its behavior, technique, indicators, and syntax details.",
        ]
    )
    for filename in sorted(files):
        purposes = questions_by_file.get(Path(filename).name, [])
        purpose = "; ".join(purposes) if purposes else "Supporting sample in the static-analysis bundle."
        content_hash = hashlib.sha256(files[filename].encode("utf-8")).hexdigest()
        lines.append(f"- `{filename}` — {purpose} SHA-256 `{content_hash}`.")
    lines.extend(["", "## Questions and answer formats", ""])
    for question in attempt.get("questions", []):
        lines.extend(
            [
                f"### `{question.get('id', '')}`",
                "",
                question.get("question", ""),
                "",
                f"- **Answer ID:** `{question.get('id', '')}`",
                f"- **Artifact:** `{question.get('artifact', '')}`",
                f"- **Answer format:** `{json.dumps(question.get('answer_format', {}), ensure_ascii=False)}`",
                f"- **Normalization:** `{question.get('normalization', '')}`",
                "",
            ]
        )
    lines.extend(
        [
            "## How to submit",
            "",
            f"Submit every answer in the `answers` object using the exact answer ID above. The solver writes the derived answer set to `{answers_path.name}` and sends it through the Game Master proxy.",
            "A successful mode submission is the Game Master's `CORRECT` result.",
            "",
        ]
    )
    report_path.write_text("\n".join(lines))
    return {
        "archive": str(archive_path),
        "sha256": digest,
        "start": str(start_path),
        "answers": str(answers_path),
        "report": str(report_path),
        "result_file": str(result_path) if result is not None else "",
        "delivery_files": json.dumps(delivery_paths),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--challenge", default=DEFAULT_CHALLENGE)
    parser.add_argument("--archive", type=Path, help="an already downloaded assigned ZIP")
    parser.add_argument("--output-dir", type=Path, default=Path("artifacts"))
    parser.add_argument(
        "--retry-expired",
        action="store_true",
        help="spend a retry for an expired timed attempt before solving",
    )
    parser.add_argument("--answers-only", action="store_true", help="print answers without submitting")
    args = parser.parse_args()

    attempt = retry_attempt(args.challenge) if args.retry_expired else start_attempt(args.challenge)
    deliveries: list[tuple[dict[str, Any], bytes]] | None = None
    if args.archive:
        archive = args.archive.read_bytes()
        files = read_archive(archive)
    elif attempt.get("mode") == "mastery" and attempt.get("artifacts"):
        deliveries = []
        files = {}
        for index, artifact_info in enumerate(attempt["artifacts"], 1):
            content = download_capability(artifact_info["download"])
            actual_hash = hashlib.sha256(content).hexdigest()
            advertised_hash = artifact_info.get("sha256")
            if advertised_hash and advertised_hash.lower() != actual_hash:
                raise SystemExit(
                    f"Delivery {index} hash mismatch: Game Master advertised {advertised_hash}, got {actual_hash}"
                )
            deliveries.append((artifact_info, content))
            extracted = read_archive(content)
            files.update({f"set{index}/{name}": text for name, text in extracted.items()})
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_STORED) as local_bundle:
            for index, (_, content) in enumerate(deliveries, 1):
                local_bundle.writestr(f"evidence-{index:02d}.zip", content)
        archive = buffer.getvalue()
    else:
        archive = download_archive(args.challenge)
        advertised_hash = attempt.get("sha256") or (
            (attempt.get("artifacts") or [{}])[0].get("sha256")
        )
        actual_hash = hashlib.sha256(archive).hexdigest()
        if advertised_hash and advertised_hash.lower() != actual_hash:
            raise SystemExit(
                f"Artifact hash mismatch: Game Master advertised {advertised_hash}, got {actual_hash}"
            )
        files = read_archive(archive)
    answers = build_answers(files)
    expected = set(attempt.get("answer_ids", []))
    missing = expected - answers.keys()
    extra = answers.keys() - expected if expected else set()
    if missing or extra:
        print(json.dumps({"missing_ids": sorted(missing), "unexpected_ids": sorted(extra)}, indent=2), file=sys.stderr)
        return 2

    if args.answers_only:
        result = None
    else:
        result = submit(args.challenge, answers)
    paths = write_run_files(
        args.challenge, attempt, archive, files, answers, result, args.output_dir, deliveries
    )
    summary = {
        "result": (result or {}).get("result", "answers_only"),
        "name": attempt.get("name"),
        "mode": attempt.get("mode")
        or (args.challenge.rsplit("/", 1)[-1] if "/" in args.challenge else "single_solve"),
        "deadline": attempt.get("deadline"),
        "answers": len(answers),
        **paths,
    }
    if result:
        for key in ("remaining", "solved_by", "status", "message"):
            if key in result:
                summary[key] = result[key]
    print(json.dumps(summary, indent=2, ensure_ascii=False))
    if result and result.get("result") == "already_solved":
        return 0
    if args.answers_only:
        return 0
    return 0 if result and result.get("result") in {"CORRECT", "accepted"} else 1


if __name__ == "__main__":
    raise SystemExit(main())

