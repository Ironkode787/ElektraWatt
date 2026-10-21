-- SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
-- SPDX-License-Identifier: CC0-1.0
--
-- Golden acceptance (doc 20 §3.1, ADR-0017; api = "v1"): the cable overview and core lists of folio 906
-- (doc 20 §10; AC-11). The result equals expected/csv/cables.csv: a heading and one row per cable in
-- natural tag order, then per cable a heading and its cores (shield last, pair, natural core name). A core
-- prints the colour and cross-section of its wire, else the conductor's, and the ends of its wire.
WITH
cable AS (
  SELECT c.*, row_number() OVER (ORDER BY c.tag COLLATE ew_natural, c.id) AS rank
  FROM api_v1_cable AS c
),
core AS (
  SELECT k.*, c.rank AS rank, c.tag AS tag,
         row_number() OVER (PARTITION BY k.cable_id
                            ORDER BY k.shield, k.pair, k.core COLLATE ew_natural, k.id) AS n,
         w.colour AS wire_colour, w.csa AS wire_csa, w.csa_unit AS wire_csa_unit,
         CASE w.from_end
           WHEN 'device' THEN
             CASE WHEN tf.id IS NOT NULL THEN w.from_tag || coalesce('/' || w.from_clamp, '')
                  ELSE w.from_tag || coalesce(':' || nullif(w.from_cp, ''), '') END
           WHEN 'external' THEN 'ext. ' || w.from_ip
           WHEN 'open' THEN 'open ' || w.from_ip
         END AS end_a,
         CASE w.to_end
           WHEN 'device' THEN
             CASE WHEN tt.id IS NOT NULL THEN w.to_tag || coalesce('/' || w.to_clamp, '')
                  ELSE w.to_tag || coalesce(':' || nullif(w.to_cp, ''), '') END
           WHEN 'external' THEN 'ext. ' || w.to_ip
           WHEN 'open' THEN 'open ' || w.to_ip
         END AS end_b
  FROM api_v1_conductor AS k
  JOIN cable AS c ON c.id = k.cable_id
  LEFT JOIN api_v1_wire AS w ON w.id = k.wire_id
  LEFT JOIN api_v1_terminal AS tf ON tf.id = w.from_device_id
  LEFT JOIN api_v1_terminal AS tt ON tt.id = w.to_device_id
),
lines AS (
  SELECT 0 AS block, 0 AS k, 0 AS sub, 'heading' AS kind, 'Cable overview' AS cable, NULL AS part,
         NULL AS core, NULL AS colour, NULL AS csa, NULL AS end_a, NULL AS end_b, NULL AS length
  WHERE EXISTS (SELECT 1 FROM api_v1_cable)
  UNION ALL
  SELECT 1, c.rank, 0, 'data', c.tag, nullif(c.part_number, ''), NULL, NULL, NULL,
         (SELECT group_concat(value, ', ' ORDER BY key) FROM json_each(c.end_a)),
         (SELECT group_concat(value, ', ' ORDER BY key) FROM json_each(c.end_b)),
         CASE c.length_source
           WHEN 'manual' THEN
             -- The length in metres from nanometres, as exact decimal text.
             ((c.length_nm / 1000000) / 1000)
               || CASE WHEN (c.length_nm / 1000000) % 1000 = 0 THEN ''
                       ELSE '.' || rtrim(printf('%03d', (c.length_nm / 1000000) % 1000), '0') END
               || ' m'
           WHEN 'part' THEN 'from part'
           WHEN 'missing' THEN 'length missing'
         END
  FROM cable AS c
  UNION ALL
  SELECT 2, c.rank, 0, 'heading', c.tag || coalesce(' ' || nullif(c.part_number, ''), ''), NULL, NULL, NULL,
         NULL, NULL, NULL, NULL
  FROM cable AS c
  UNION ALL
  SELECT 2, k.rank, k.n, 'data', k.tag, NULL, k.core,
         CASE WHEN k.shield = 0 THEN coalesce(k.wire_colour, k.colour) END,
         CASE WHEN k.shield = 1 THEN NULL
              WHEN k.wire_csa IS NOT NULL THEN
                CASE k.wire_csa_unit WHEN 'mm²' THEN k.wire_csa || ' mm²' WHEN 'AWG' THEN 'AWG' || k.wire_csa
                                     WHEN 'kcmil' THEN k.wire_csa || ' kcmil' END
              ELSE
                CASE k.csa_unit WHEN 'mm²' THEN k.csa || ' mm²' WHEN 'AWG' THEN 'AWG' || k.csa
                                WHEN 'kcmil' THEN k.csa || ' kcmil' END
         END,
         k.end_a, k.end_b, NULL
  FROM core AS k
)
SELECT kind AS "row", cable AS "Cable", part AS "Cable type", core AS "Core", colour AS "Colour", csa AS "mm²",
       end_a AS "From", end_b AS "To", length AS "Length"
FROM lines
ORDER BY block, k, sub
