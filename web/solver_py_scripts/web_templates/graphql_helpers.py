#!/usr/bin/env python3
"""GraphQL helper snippets for a NEW family's own bug - a template, not a recoverer.

Only one GraphQL-shaped bug has been seen so far (day-18-resolver-revolt, release
day 25: a list query enforces field-level authorization by omission, the matching
single-object query doesn't check at all; `recovery_lib.rec_graphql_field_bypass`
hardcodes that specific app's `case`/`caseIndex`/`note` names because that is the
only confirmed shape). This module factors out the parts of that solve that
generalize to ANY GraphQL target, for whoever writes the next one:

- `graphql()`: a thin POST wrapper built on `Target.post_json` (same pacing, same
  evidence-saving, same WAF-safe UA already in `recovery_lib.Target` - do not
  build a second HTTP client, import `Target` from `recovery_lib` and pass it in).
- `INTROSPECTION_QUERY` + `list_query_field_names()`: discover the schema's Query
  fields without guessing names, so you can find both the "list" surface and the
  matching "single-object" surface by shape rather than by reading HTML by eye.
- The one general lesson this family's own WAF gotcha taught, repeated here
  because it has no other natural home: a bare `Python-urllib/*` default
  User-Agent gets 403'd by some edge WAFs with **no body hint** that it's a UA
  issue. `Target`'s default UA already covers this - never override it to
  anything urllib-identifying when hand-rolling a request to a GraphQL endpoint.

Not wired into `classify_and_recover` - a GraphQL target's actual authorization
gap is app-specific. Port the pieces you need into your own job's `solve.py`
rather than importing this as an oracle, per the discipline in
`docs/web-worker-lessons.md`.
"""

INTROSPECTION_QUERY = """
query IntrospectionQuery {
  __schema {
    queryType { name }
    mutationType { name }
    types {
      name
      kind
      fields { name args { name } type { name kind ofType { name kind } } }
    }
  }
}
"""


def graphql(t, route, query, variables=None, save=None):
    """POST a GraphQL query/mutation `query` to `route` on Target `t`.
    Returns (status, body_dict) - body_dict is the raw {"data": ..., "errors": ...}
    response, parsed JSON (or None if the body wasn't valid JSON)."""
    payload = {"query": query}
    if variables is not None:
        payload["variables"] = variables
    return t.post_json(route, payload, save=save)


def list_query_field_names(introspection_body):
    """Every field name on the schema's Query type, from a successful
    introspection response (as returned by `graphql(t, route, INTROSPECTION_QUERY)`).
    Use this to spot the "list" surface and its matching "single-object" surface
    by name/shape (e.g. `caseIndex` vs `case`) instead of reading the schema by eye.
    Returns [] if the body doesn't look like a successful introspection response."""
    data = (introspection_body or {}).get("data") or {}
    schema = data.get("__schema") or {}
    query_type_name = (schema.get("queryType") or {}).get("name")
    for type_entry in schema.get("types", []):
        if type_entry.get("name") == query_type_name:
            return [f["name"] for f in type_entry.get("fields", [])]
    return []
