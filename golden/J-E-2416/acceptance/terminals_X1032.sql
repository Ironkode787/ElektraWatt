-- SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
-- SPDX-License-Identifier: CC0-1.0
--
-- Golden acceptance (doc 20 §3.1, ADR-0017; api = "v1"): the terminal diagram of strip -X1032 on folio
-- 905 (doc 20 §9; AC-08). The result equals expected/csv/terminals_X1032.csv: one row per terminal in strip
-- order; a side lists the far end of every wire at the terminal's CPs of that side, with the cable core
-- of a core wire.
WITH
ends AS (
  -- Every wire end, seen from that end: the CP side there and the far end.
  SELECT w.id AS wire, w.from_device_id AS at, w.from_side AS side, w.to_end AS far_end,
         w.to_device_id AS far_device, w.to_tag AS far_tag, w.to_cp AS far_cp, w.to_clamp AS far_clamp,
         w.to_ip AS far_ip, w.cable_tag AS cable_tag, w.core AS core
  FROM api_v1_wire AS w
  WHERE w.kind = 'wire'
  UNION ALL
  SELECT w.id, w.to_device_id, w.to_side, w.from_end, w.from_device_id, w.from_tag, w.from_cp, w.from_clamp,
         w.from_ip, w.cable_tag, w.core
  FROM api_v1_wire AS w
  WHERE w.kind = 'wire'
),
targets AS (
  -- The far end as text: a terminal with its clamp (-X1031:12/b), another device with its CP
  -- (-201U1:5), an external point (ext. J-E-206-L); then the cable core.
  SELECT e.wire AS wire, e.at AS at, e.side AS side,
         CASE e.far_end
           WHEN 'device' THEN
             CASE WHEN ft.id IS NOT NULL THEN e.far_tag || coalesce('/' || e.far_clamp, '')
                  ELSE e.far_tag || coalesce(':' || nullif(e.far_cp, ''), '') END
           WHEN 'external' THEN 'ext. ' || e.far_ip
           ELSE 'open ' || e.far_ip
         END || coalesce(' via ' || e.cable_tag || ' ' || e.core, '') AS text
  FROM ends AS e
  LEFT JOIN api_v1_terminal AS ft ON ft.id = e.far_device
)
SELECT 'data' AS "row",
       t.designation AS "Terminal",
       t.function_text AS "Function text",
       t.strip_group AS "Group",
       CASE WHEN t.part_number IN ('', 'TBD') THEN 'TBD' ELSE t.part_number END AS "Part",
       t.potential_name AS "Potential",
       (SELECT group_concat(x.text, ', ' ORDER BY x.wire) FROM targets AS x WHERE x.at = t.id AND x.side = 1)
         AS "Side 1 (clamp a)",
       (SELECT group_concat(x.text, ', ' ORDER BY x.wire) FROM targets AS x WHERE x.at = t.id AND x.side = 2)
         AS "Side 2 (clamp b)",
       t.bridge AS "Bridge",
       t.state AS "State"
FROM api_v1_terminal AS t
WHERE t.strip_tag = '-X1032'
ORDER BY t.seq, t.id
