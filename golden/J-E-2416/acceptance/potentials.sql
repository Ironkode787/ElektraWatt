-- SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
-- SPDX-License-Identifier: CC0-1.0
--
-- Golden acceptance (doc 20 §3.1, ADR-0017; api = "v1"): the potential overview of folio 909 (doc 20 §7.3,
-- §7.4; AC-10). The result equals expected/csv/potentials.csv: one row per potential in natural order of
-- its name, then one row per bond.
WITH
pages AS (
  -- The folios of every potential and bond with their position in the page order.
  SELECT p.id AS id, j.key AS i, j.value AS folio, g.physical_index AS pos
  FROM api_v1_potential AS p, json_each(p.pages) AS j
  JOIN api_v1_page AS g ON g.folio = j.value
  UNION ALL
  SELECT b.id, j.key, j.value, g.physical_index
  FROM api_v1_bond AS b, json_each(b.pages) AS j
  JOIN api_v1_page AS g ON g.folio = j.value
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
  SELECT 0 AS block, p.name AS k1, p.id AS k2, 'data' AS kind, p.name AS potential, p.ptype AS type,
         CASE WHEN p.voltage IS NOT NULL
              THEN p.voltage || CASE p.ac_dc WHEN 'ac' THEN ' V AC' WHEN 'dc' THEN ' V DC' ELSE ' V' END
         END AS voltage,
         p.circuit_class AS class,
         -- A bus line without a profile value takes colour and size from its cable: "(cable)".
         CASE WHEN p.colour IS NULL AND json_array_length(p.bus_cable_ids) > 0 THEN '(cable)'
              ELSE p.colour END AS colour,
         CASE WHEN p.default_csa IS NULL AND json_array_length(p.bus_cable_ids) > 0 THEN '(cable)'
              ELSE CASE p.default_csa_unit WHEN 'mm²' THEN p.default_csa || ' mm²'
                                           WHEN 'AWG' THEN 'AWG' || p.default_csa
                                           WHEN 'kcmil' THEN p.default_csa || ' kcmil' END
         END AS csa,
         json_array_length(p.aliases) AS aliases,
         p.member_count AS members,
         f.text AS pages
  FROM api_v1_potential AS p
  LEFT JOIN folios AS f ON f.id = p.id
  UNION ALL
  SELECT 1, NULL, CAST(substr(b.label, 2) AS INTEGER), 'data',
         b.label || ' (' || coalesce(b.a_potential_name, '?') || ' ⇄ ' || coalesce(b.b_potential_name, '?') || ')',
         'Bond', NULL, NULL, NULL, NULL, NULL, NULL, f.text
  FROM api_v1_bond AS b
  LEFT JOIN folios AS f ON f.id = b.id
)
SELECT kind AS "row", potential AS "Potential", type AS "Type", voltage AS "Voltage", class AS "Class",
       colour AS "Colour", csa AS "mm²", aliases AS "Aliases", members AS "Members", pages AS "Pages"
FROM lines
ORDER BY block, k1 COLLATE ew_natural, k2
