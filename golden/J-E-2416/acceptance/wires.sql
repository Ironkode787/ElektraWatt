-- SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
-- SPDX-License-Identifier: CC0-1.0
--
-- Golden acceptance (doc 20 §3.1, ADR-0017; api = "v1"): the wire list of folio 907 (doc 20 §8.2, §8.3;
-- AC-09). The result equals expected/csv/wires.csv: one row per physical wire in reading order, then a
-- heading and the terminal bridges.
WITH
pages AS (
  -- The folios of every wire with their position in the page order.
  SELECT w.id AS id, j.key AS i, j.value AS folio, p.physical_index AS pos
  FROM api_v1_wire AS w, json_each(w.pages) AS j
  JOIN api_v1_page AS p ON p.folio = j.value
),
islands AS (
  SELECT id, i, folio, pos - row_number() OVER (PARTITION BY id ORDER BY i) AS run FROM pages
),
runs AS (
  SELECT id, run, min(i) AS lo, max(i) AS hi, count(*) AS n, group_concat(folio, ', ' ORDER BY i) AS list
  FROM islands
  GROUP BY id, run
),
folios AS (
  -- Three or more pages consecutive in the page order print "first–last", shorter runs as a list.
  SELECT r.id AS id,
         group_concat(CASE WHEN r.n >= 3 THEN a.folio || '–' || b.folio ELSE r.list END, ', ' ORDER BY r.lo)
           AS text
  FROM runs AS r
  JOIN islands AS a ON a.id = r.id AND a.i = r.lo
  JOIN islands AS b ON b.id = r.id AND b.i = r.hi
  GROUP BY r.id
),
lines AS (
  SELECT 0 AS block, w.seq AS k, 'data' AS kind,
         w.number AS no,
         CASE w.from_end WHEN 'device' THEN w.from_tag WHEN 'external' THEN 'ext. ' || w.from_ip
                         ELSE 'open ' || w.from_ip END AS from_end,
         CASE WHEN w.from_end = 'device' THEN nullif(w.from_cp, '') END AS from_cp,
         CASE w.to_end WHEN 'device' THEN w.to_tag WHEN 'external' THEN 'ext. ' || w.to_ip
                       ELSE 'open ' || w.to_ip END AS to_end,
         CASE WHEN w.to_end = 'device' THEN nullif(w.to_cp, '') END AS to_cp,
         w.potential_name AS potential,
         w.colour AS colour,
         CASE w.csa_unit WHEN 'mm²' THEN w.csa || ' mm²' WHEN 'AWG' THEN 'AWG' || w.csa
                         WHEN 'kcmil' THEN w.csa || ' kcmil' END AS csa,
         CASE WHEN w.csa_source IS NULL THEN w.colour_source
              WHEN w.colour_source IS NULL OR w.colour_source = w.csa_source THEN w.csa_source
              ELSE w.csa_source || '/' || w.colour_source END AS source,
         nullif(concat_ws('; ',
                          CASE WHEN w.core IS NOT NULL THEN w.cable_tag || ' ' || w.core END,
                          -- The junction by its place and wiring (doc 08 §3.2, D-20).
                          CASE WHEN w.junction_style IS NOT NULL THEN
                            w.junction_cell || ' ' || w.junction_wiring
                            || coalesce((SELECT CASE w.junction_wiring WHEN 'star' THEN ' at ' ELSE ' on ' END
                                                || group_concat(value, ', ' ORDER BY key)
                                         FROM json_each(w.junction_cps)), '')
                          END,
                          -- Daisy chains: two conductors on one clamp (FR-CON-006).
                          (SELECT 'chain on ' || group_concat(value, ', ' ORDER BY key) FROM json_each(w.chain_cps)),
                          (SELECT 'IP ' || group_concat(value, ', ' ORDER BY key) FROM json_each(w.ips))),
                '') AS via,
         CASE WHEN w.wire_part IN ('', 'TBD') THEN 'TBD' ELSE w.wire_part END AS part,
         f.text AS pages
  FROM api_v1_wire AS w
  LEFT JOIN folios AS f ON f.id = w.id
  WHERE w.kind = 'wire'
  UNION ALL
  SELECT 1, 0, 'heading', 'Bridges', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL
  WHERE EXISTS (SELECT 1 FROM api_v1_wire WHERE kind = 'bridge')
  UNION ALL
  SELECT 2, b.seq, 'data', b.number, b.from_tag, NULL, b.to_tag, NULL, b.potential_name, NULL, NULL, NULL,
         b.bridge_kind || ' bridge, terminals '
           || CASE WHEN b.from_device_id = b.to_device_id THEN ta.designation
                   ELSE ta.designation || '–' || tz.designation END,
         'TBD',
         f.text
  FROM api_v1_wire AS b
  LEFT JOIN api_v1_terminal AS ta ON ta.id = b.from_device_id
  LEFT JOIN api_v1_terminal AS tz ON tz.id = b.to_device_id
  LEFT JOIN folios AS f ON f.id = b.id
  WHERE b.kind = 'bridge'
)
SELECT kind AS "row", no AS "No.", from_end AS "From", from_cp AS "CP", to_end AS "To", to_cp AS "CP",
       potential AS "Potential", colour AS "Colour", csa AS "mm²", source AS "Source", via AS "Via",
       part AS "Wire part", pages AS "Pages"
FROM lines
ORDER BY block, k
