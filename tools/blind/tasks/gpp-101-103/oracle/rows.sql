-- The read-only queries of the oracle of the task gpp-101-103 (WP-S6.13 §5.7, §5.9).
--
-- Each query begins at a line `-- query: <name>`. The harness runs them with the copied `ew sql` over a run's
-- `work/project` (the scoring) and `bless-oracle` runs them over a temporary copy of Golden A (the expected rows in
-- `rows/*.csv`). No query selects an ID: a new project mints its own, so rows hold tags, labels, cells and texts only.
-- Folios are 101 to 103 (doc 20 §5). Anchor cells (S1) and junction sequences (S3) are not read from SQL: `api_v1_function`
-- carries only a device's first representation, so both come from the `page.describe` texts.

-- query: ends
-- S2: the ends of the physical wires that lie on folios 101 to 103 only, up to orientation (the smaller label first).
-- A terminal end prints `<terminal tag>/<clamp>`, a device end `<tag>:<connection point>`, an external end `ext <name>`.
WITH w AS (
  SELECT json_extract(pages, '$[0]') AS folio,
         CASE WHEN from_end = 'device'
              THEN CASE WHEN from_clamp IS NOT NULL THEN from_tag || '/' || from_clamp ELSE from_tag || ':' || from_cp END
              ELSE 'ext ' || from_ip END AS a,
         CASE WHEN to_end = 'device'
              THEN CASE WHEN to_clamp IS NOT NULL THEN to_tag || '/' || to_clamp ELSE to_tag || ':' || to_cp END
              ELSE 'ext ' || to_ip END AS b,
         (SELECT group_concat(value, ',') FROM json_each(pages)) AS span
  FROM api_v1_wire
  WHERE kind = 'wire'
    AND NOT EXISTS (SELECT 1 FROM json_each(pages) WHERE value NOT IN ('101', '102', '103'))
)
SELECT folio,
       CASE WHEN a <= b THEN a ELSE b END AS end_1,
       CASE WHEN a <= b THEN b ELSE a END AS end_2,
       span
FROM w
ORDER BY 1, 2, 3;

-- query: xrefs
-- S4: the printed cross-reference of every function on folios 101 to 103 that has one (interruption points and
-- external points included), by the function's tag or interruption-point name.
SELECT folio, coalesce(device_tag, ip_name) AS tag, xref_text AS xref
FROM api_v1_function
WHERE folio IN ('101', '102', '103')
  AND xref_text IS NOT NULL
  AND fndef <> 'ew.fn.strip-segment'
ORDER BY folio, tag;

-- query: tags
-- S5: the tags of the functions on folios 101 to 103 (frozen tags included) and the names of the interruption points.
-- A strip segment is left out, and so are the terminals of folio 103: the demo variant places them one by one (H3
-- counts them).
SELECT folio, kind, coalesce(device_tag, ip_name) AS tag
FROM api_v1_function
WHERE folio IN ('101', '102', '103')
  AND fndef <> 'ew.fn.strip-segment'
  AND NOT (folio = '103' AND fndef LIKE 'ew.fn.terminal%')
ORDER BY folio, kind, tag;

-- query: pages
-- S6: the titles of folios 101 to 103.
SELECT folio, title
FROM api_v1_page
WHERE folio IN ('101', '102', '103')
ORDER BY physical_index;

-- query: terminals
-- H3: the terminals and the strips on folio 103.
SELECT count(*) AS terminals, count(DISTINCT t.strip_tag) AS strips
FROM api_v1_terminal AS t
JOIN api_v1_page AS p ON p.id = t.page_id
WHERE p.folio = '103';

-- query: placed_terminals
-- bless-oracle only: the terminals that a drawn connection of folio 103 ends at, which the demo variant of folio 103
-- places one by one (WP-S2.04 §13.6), with their strip, part number and bridge.
SELECT t.strip_tag AS strip, t.designation AS designation, coalesce(t.part_number, '') AS part, coalesce(t.bridge, '') AS bridge
FROM api_v1_terminal AS t
WHERE EXISTS (
  SELECT 1
  FROM api_v1_wire AS w, json_each(w.pages) AS j
  WHERE w.kind = 'wire' AND j.value = '103' AND (w.from_device_id = t.id OR w.to_device_id = t.id)
)
ORDER BY t.strip_tag, t.seq;
