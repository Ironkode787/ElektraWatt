// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The example lines every test shares: the doc 05 §3.5 examples verbatim, the doc 05 §3.5 part copy
//! (compacted), one or more golden lines per implemented record kind and one golden document per library
//! kind. `@TOKEN`s are filled by [`super::fill`].

/// Doc 05 §3.5, verbatim.
pub const DOC05_LINES: &[&str] = &[
    r#"{"$k":"page","id":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","folio":{"section":"0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e","number":101},"kind":"multi_line","structure":{"function":"7d2c95e0-1f4a-5b38-a6e7-0c93b5d1f284","location":"a41f6c2d-8e05-5d97-b3c1-5f2e8a7d9c60"},"title":{"en-GB":"Power Supply 1 - Main Current"},"grid":{"num":7937500,"den":3}}"#,
    r#"{"$k":"representation","id":"6f03d8a5-c912-5e7b-8a40-1d5b39e2f7c6","functions":["3e7a1c58-90d4-5b26-8f3a-6c0d2e9b417e"],"page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","symbol":"2c7e9b40-5a18-5f63-9d2c-b8a1e07f4396","pos":[137583333,31750000]}"#,
    r#"{"$k":"connection","id":"b29e4f17-8d63-5a0c-9f71-3c6a2e8d05b1","a":{"cp":["d40b6e93-25f7-5c1a-9e8d-7a3f1c05b6e2",0]},"b":{"cp":["3e7a1c58-90d4-5b26-8f3a-6c0d2e9b417e",0]},"page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","route":[[42333333,15875000],[137583333,15875000]],"overrides":{"csa":"2.5","colour":["BK"],"show_marker":true}}"#,
    r#"{"$k":"device","id":"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2","tag":{"letter":"G","counter":1},"kind":"psu","parts":[{"part":{"copy":"e5d2a8f1-0c37-5b94-8e6a-4f1d93c7b025"},"qty":"1"}],"function_text":{"en-GB":"Power supply 1 AC / 24 V DC"},"tech_data":[{"key":"i_out","value":{"value":"20","unit":"A"}}]}"#,
    r#"{"$k":"function","id":"1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19","device":"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2","def":"5b9e2d70-3c41-5f8a-b6d2-19e4c7a03f58","kind":"black_box"}"#,
    r#"{"$k":"device","id":"f19c3a7e-6b02-5e48-9d5a-c7e1340b8f2a","tag":{"letter":"X"},"kind":"terminal","parts":[{"part":{"copy":"a07c4e19-d25b-5f83-9c16-4e8b0d7a2f35"},"qty":"1"}],"function_text":"1+","terminal":{"strip":"58e2b1d0-4c97-5a3f-8e16-d09a72c5e3f1","seq":1,"designation":"1"}}"#,
];

/// The doc 05 §3.5 part copy, compacted; IDs and hash abbreviated in the document are tokens here.
pub const DOC05_PART: &str = r#"{"$k":"part","id":"@PART_PSU","library_id":"@LIB","item_id":"@ITEM_PSU","version":"1.2.0","sha256":"@HASH","meta":{"licence":"CC0-1.0","sources":["golden tables"]},"manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^1"},"part_number":"2904602","description":{"en-GB":"Power supply 1 AC / 24 V DC, 20 A"},"cp_pattern":[{"idx":0,"designation":"L"},{"idx":1,"designation":"N"},{"idx":2,"designation":"PE","potential_role":"pe"},{"idx":3,"designation":"+","potential_role":"l_plus"},{"idx":4,"designation":"+","potential_role":"l_plus"},{"idx":5,"designation":"-","potential_role":"m"},{"idx":6,"designation":"-","potential_role":"m"}]}"#;

/// Golden lines, one or more per implemented record kind (doc 20 values).
pub const GOLDEN: &[&str] = &[
    // project (doc 20 §4.1, §4.2): only the golden party placeholders.
    r#"{"$k":"project","id":"@PRJ","master":{"description":{"en-GB":"Expansion Control Cabinets"},"project_ref":"<project ref>","drawing_id":"<drawing id>","object_id":"J-E-2416","manufacturing_year":2025,"created":"2025-03-03","edited":"2025-04-14"},"parties":[{"role":"customer","name":"<customer>"},{"role":"end_user","name":"<end user>"},{"role":"engineer","name":"<engineer>","personal_data":true}],"supply":{"phases":1,"neutral":true,"pe":true,"earthing":"tn_s","u_nom":"230","frequency":"50"},"control_voltage":{"value":"24","unit":"V"},"rated_consumption":{"value":"6","unit":"A"},"settings":{"identifier_set":"EN81346-2:2009","ip_pairing":{"mode":"star","scope":"project"},"formatting":{"date":"dd_mm_yyyy","decimal_separator":"point"}},"languages":{"source":"en-GB"},"derivation_profile":"2027.1"}"#,
    // structure (doc 20 §4.3)
    r#"{"$k":"structure_node","id":"@LOC_JE2416","aspect":"location","code":"J-E-2416","aliases":["JE2416"],"description":{"en-GB":"This cabinet"},"order":1}"#,
    r#"{"$k":"structure_node","id":"@FN_P01","aspect":"function","code":"P01"}"#,
    r#"{"$k":"section","id":"@SEC1","number":1,"folio_range":[100,199],"title":{"en-GB":"Wiring Definition"},"divider":true}"#,
    r#"{"$k":"external_ref","id":"@EXT_L","location":"@LOC_JE206","device":"JE-N1000-03","connection":"L"}"#,
    // pages (doc 20 §5)
    r#"{"$k":"page","id":"@P1","folio":{"number":1},"kind":"title_page","structure":{"function":"@FN_P01","location":"@LOC_JE2416"},"form":"@FORM_COVER","title":{"en-GB":"Title Page"},"grid":{"num":7937500,"den":3}}"#,
    r#"{"$k":"page","id":"@P902","folio":{"section":"@SEC9","number":902},"kind":"panel_layout","structure":{"function":"@FN_P01","location":"@LOC_JE2416"},"title":{"en-GB":"Cabinet Layout"},"scale":{"one_to":10},"grid":{"num":7937500,"den":3}}"#,
    r#"{"$k":"page","id":"@P103","folio":{"section":"@SEC1","number":103,"label":"103a"},"kind":"multi_line","structure":{"document_kind":"none"},"title":{"en-GB":"Power Supply 3 - Control Current Distribution"},"description_override":{"en-GB":"Control Cabinet Expansion"},"grid":{"num":7937500,"den":3}}"#,
    // devices (doc 20 §6, §9, §10)
    r#"{"$k":"device","id":"@DEV_S51","tag":{"letter":"S","counter":1,"frozen":{"text":"S51","sign":"hidden"}},"parts":[{"part":{"copy":"@PART_533356"},"qty":"1"}],"tech_data":[{"key":"curve","value":"C"},{"key":"i_rated","value":{"value":"40","unit":"A"}}]}"#,
    r#"{"$k":"device","id":"@DEV_X1031","tag":{"letter":"X","counter":1},"function_text":{"en-GB":"24 V / 0 V distribution"},"strip":{"label_mode":"function_text","groups":[{"from_seq":1,"to_seq":4,"label":{"en-GB":"Pneumatics"}},{"from_seq":5,"to_seq":10,"label":{"en-GB":"Spare"}}]}}"#,
    r#"{"$k":"device","id":"@DEV_X1032_1","tag":{"letter":"X"},"kind":"terminal","parts":[{"part":{"copy":"@PART_TWINPE"},"qty":"1"}],"function_text":"PE","state":"spare","terminal":{"strip":"@DEV_X1031","seq":5,"designation":"5","levels":[{"level":1}],"rail_bonded":true}}"#,
    r#"{"$k":"device","id":"@DEV_W106_1","tag":{"letter":"W","counter":1},"parts":[{"part":{"copy":"@PART_NEBU"},"qty":"1"}],"cable":{"length":"from_part","core_id_mode":"colour"}}"#,
    r#"{"$k":"device","id":"@DEV_W3_1","tag":{"letter":"W","counter":1},"cable":{}}"#,
    r#"{"$k":"device","id":"@DEV_G1","tag":{"letter":"G","counter":1},"cable":{"length":{"manual":2000000000},"pre_assembled":true}}"#,
    // the assembly aspect (doc 20 §14.2, D-41)
    r#"{"$k":"device","id":"@DEV_G1","tag":{"letter":"U","counter":1},"parts":[{"part":{"copy":"@PART_NEBU"},"qty":"1"}],"assembly":{"configs":[{"label":{"en-GB":"Order Code 1"},"order_code":"51E-F13GCQP-D+H2GG","part":{"copy":"@PART_NEBU"},"covers":["@DEV_G1"]},{"label":{"en-GB":"Order Code 2"},"order_code":"32P-VGL-R","covers":["@DEV_S51","@DEV_X1031"]}],"bom_basis":"components"},"plc_station":{"config_project":"@PLC1","role":"valve_terminal"}}"#,
    // PLC aspects (doc 20 §11.2)
    r#"{"$k":"device","id":"@DEV_G1","tag":{"letter":"U","counter":1},"plc_station":{"config_project":"@PLC1","role":"valve_terminal"}}"#,
    r#"{"$k":"device","id":"@DEV_S51","tag":{"letter":"U","counter":3},"parent":"@DEV_G1","position":2,"plc_module":{}}"#,
    r#"{"$k":"device","id":"@DEV_S51","tag":{"letter":"U","counter":3},"plc_module":{"slot":{"rack":{"rack":0,"slot":3,"subslot":1}},"start":{"input":"tbd","output":{"manual":{"area":"q","byte":10}}},"part_use":1,"integrated":true}}"#,
    // functions (doc 20 §6, §8)
    r#"{"$k":"function","id":"@FN_IP24","def":"@FNDEF_IP","kind":{"ip":{"name":"24VDC-1","direction":"source","star_source":true}}}"#,
    r#"{"$k":"function","id":"@FN_F2_102","device":"@DEV_G1","def":"@FNDEF_MCB","role":"aux","cp_overrides":[{"idx":0,"designation":"1","alias":"L1"},{"idx":3,"unused":true}]}"#,
    r#"{"$k":"function","id":"@FN_X1031","device":"@DEV_X1031","def":"@FNDEF_IP","kind":{"ip":{"name":"J-E-206-L","direction":"target","external":"@EXT_L"}}}"#,
    // representations
    r#"{"$k":"representation","id":"@REP_X1031A","functions":["@FN_X1031"],"page":"@P103","symbol":"@SYM_MCB","variant":1,"pos":[23812500,23812500],"orient":"r90","rep_type":"strip_segment","pin_map":[{"symbol_cp":0,"function":"@FN_X1031","idx":1}],"text_slots":[{"slot":0,"pos":[0,-2645833],"visible":true}],"arrangement":"full"}"#,
    r#"{"$k":"representation","id":"@REP_F1","functions":["@FN_G1"],"page":"@P102","symbol":"@SYM_MCB","pos":[95250000,63500000],"box":{"outline":[[0,0],[47625000,0],[47625000,31750000],[0,31750000]],"cps":[{"idx":0,"pos":[0,7937500],"dir":"left"}]}}"#,
    // connections (doc 20 §8)
    r#"{"$k":"connection","id":"@CON_W20","a":{"cp":["@FN_G1",3]},"b":{"cp":["@FN_F1",0]},"page":"@P102","route":[[95250000,39687500],[111125000,39687500]],"overrides":{"csa":"1.5","show_marker":true},"number":{"assigned":{"counter":1}}}"#,
    r#"{"$k":"connection","id":"@CON_W29","a":{"cp":["@FN_F2_102",8]},"b":{"cp":["@FN_IP24",0]},"page":"@P102","route":[[150812500,95250000],[150812500,166687500]],"number":{"frozen":"102.5"}}"#,
    r#"{"$k":"connection","id":"@CON_BR1","a":{"cp":["@FN_X1031_1",3]},"b":{"cp":["@FN_X1031_2",3]},"kind":{"bridge":{"kind":"plug_in","group":1,"shaft":1}}}"#,
    r#"{"$k":"connection","id":"@CON_W39","a":{"cp":["@FN_X1031_1",2]},"b":{"junction":"@JN_J5"},"page":"@P103","kind":{"core":"@COND_BN"}}"#,
    r#"{"$k":"connection","id":"@CON_DP","a":{"cp":["@FN_S51",0]},"b":{"cp":["@FN_F1",1]},"page":"@P1","kind":{"bus":{"cable":"@DEV_W3_1"}}}"#,
    r#"{"$k":"connection","id":"@CON_W27","a":{"cp":["@FN_G1",5]},"b":{"cp":["@FN_X1032_2",0]},"page":"@P102","kind":"overview","overrides":{"colour":["GN","YE"],"wire_part":{"copy":"@PART_TWIN","variant":"GNYE"},"flags":{"shielded":true},"length":350000000}}"#,
    // junctions (doc 20 §8.3)
    r#"{"$k":"junction","id":"@JN_J5","page":"@P102","at":[103187500,111125000],"style":{"bevel":{"orient":"r90"}},"wiring":"star","sequence":["@CON_W20","@CON_W22","@CON_W27B"]}"#,
    r#"{"$k":"junction","id":"@JN_J1","page":"@P101","at":[103187500,55562500],"style":"dot","sequence":["@CON_W7","@CON_W20","@CON_W29"]}"#,
    // conductors (doc 20 §10)
    r#"{"$k":"conductor","id":"@COND_BN","cable":"@DEV_W106_1","core":"BN","colour":["BN"],"csa":"0.75"}"#,
    r#"{"$k":"conductor","id":"@COND_SH","cable":"@DEV_W3_1","core":"SH","shield":true}"#,
    // PLC records (doc 20 §11.2, §11.3, §6.2)
    r#"{"$k":"plc_config_project","id":"@PLC1","name":"PLC1","profile":"siemens-tia-intl@2027.1"}"#,
    r#"{"$k":"channel","id":"@CH_U3_1","module":"@DEV_S51","number":1,"kind":"do","binding":{"cp":["@FN_S51",5]}}"#,
    r#"{"$k":"channel","id":"@CH_U3_1","module":"@DEV_S51","number":0,"designation":"DO 0","kind":"di","address":{"manual":{"area":"i","size":"word","byte":64}},"state":"deactivated","symbol":"TA05_V_15","comment":{"en-GB":"Valve"},"target":"@FN_S51"}"#,
    r#"{"$k":"device_link","id":"@DL_Q1","from":"@DEV_S51","to":"@DEV_G1","kind":"drives"}"#,
    // potentials and the bond (doc 20 §7.3, §7.4)
    r#"{"$k":"potential_def","id":"@POT_LS","at":{"cp":["@FN_S51",0]},"name":"L-S"}"#,
    r#"{"$k":"potential_def","id":"@POT_24","at":{"cp":["@FN_F2_102",8]},"name":"24VDC-1","ptype":"l_plus","voltage":"24","env":{"ac_dc":"dc","earthing":"pelv"},"circuit_class":"control","props":{"csa":"0.75","colour":["DBU"]}}"#,
    r#"{"$k":"bond","id":"@BOND_B1","a":{"cp":["@FN_G1",5]},"b":{"cp":["@FN_X1032_2",0]},"kind":"pelv_0v_pe","via":"@CON_W27"}"#,
    // report blocks (doc 20 §16, doc 09 §2.2)
    r#"{"$k":"report_block","id":"@RB_TOC","source":{"kind":"toc"},"form":"@FORM_TOC","first_folio":2,"title":{"en-GB":"Table of Contents {n}"}}"#,
    r#"{"$k":"report_block","id":"@RB_903","source":{"kind":"terminal_diagram"},"manual":{"exclude":["@DEV_X1032_1"]},"sort":[{"key":"seq"},{"key":"level","descending":true}],"group":{"key":"strip","break":"new_page"},"form":"@FORM_TOC","section":"@SEC9","first_folio":903,"continuation":"next_folio","title":{"en-GB":"Terminal Diagram {group}"},"empty":{"one_page":{"en-GB":"No entries"}}}"#,
    r#"{"$k":"report_block","id":"@RB_PD201","source":{"kind":"part_data"},"form":"@FORM_TOC","first_folio":201,"embedded":{"page":"@P201","rect":[[10000000,180000000],[120000000,270000000]]}}"#,
    // revisions (doc 20 §4.1 U-04, doc 03 §18): a historic released one without archive, an open one
    r#"{"$k":"revision","id":"@REV_V1","label":"V1.0","date":"2025-03-03","description":"Initial issue","state":"released","source_ref":{"lineage":"@LIN","key":"revisions|V1.0"}}"#,
    r#"{"$k":"revision","id":"@REV_V2","label":"V2.0","date":"2025-04-14","author":"engineer","description":{"en-GB":"Changes on 101 and 106"},"state":"open","page_notes":[{"page":"@P1"},{"page":"@P103","description":{"en-GB":"Supply moved"}}]}"#,
    // page items and a structure box (doc 03 §5, §6; WP-S2.04 §10.1): every typed variant and shape kind, a
    // bound text (an unknown variant is `records::an_unknown_page_item_variant_is_kept_verbatim`)
    r#"{"$k":"page_item","id":"@PI_MARKER","page":"@P102","kind":{"wire_marker":{"connections":["@CON_W20","@CON_W29"],"at":[90000000,24000000],"angle":90000,"fields":["csa","colour"]}}}"#,
    r#"{"$k":"page_item","id":"@PI_RECT","page":"@P101","layer":"notes","kind":{"graphic":{"shape":{"rect":{"min":[20000000,40000000],"max":[80000000,64000000]}},"stroke":{"width":500000},"dash":"dashed","fill":true}}}"#,
    r#"{"$k":"page_item","id":"@PI_ARC","page":"@P101","kind":{"graphic":{"shape":{"arc":{"centre":[30000000,30000000],"r":5000000,"start":90000,"sweep":-180000}}}}}"#,
    r#"{"$k":"page_item","id":"@PI_TEXT","page":"@P101","kind":{"free_text":{"text":"40x60","at":[50800000,30480000],"angle":90000,"style":"text.small","align":"centre"}}}"#,
    r#"{"$k":"page_item","id":"@PI_BOUND","page":"@P1","kind":{"free_text":{"bind":"project.description","at":[20000000,20000000]}}}"#,
    r#"{"$k":"structure_box","id":"@SBOX_206","page":"@P101","outline":[[0,140000000],[60000000,140000000],[60000000,170000000],[0,170000000]],"style":"dotted","caption":"External Connection","assigns":{"location":"@LOC_JE206"},"external":"@EXT_L"}"#,
    // import lineage (doc 14 §2.3); its namespace is checked in its own test
    r#"{"$k":"import_lineage","id":"@LIN","importer":"csv","source_name":"tables","anchor":{"fixed":"golden"},"namespace":"@PRJ","last_source_sha256":"@HASH"}"#,
];

/// Golden library documents, one per implemented library kind.
pub const GOLDEN_LIB: &[&str] = &[
    r#"{"$k":"fndef","id":"@FNDEF_MCB","library_id":"@LIB","item_id":"@ITEM_FN","version":"1.0.0","sha256":"@HASH","meta":{"key":"ew.iec.mcb-1pn","licence":"CC-BY-SA-4.0","confidence":{"cps":"verified"}},"category":["ProtectionOvercurrent","MCB","1P+N"],"contact_kind":"pole","cps":[{"idx":0,"designation":"1"},{"idx":1,"designation":"2"},{"idx":2,"designation":"N"},{"idx":3,"designation":"N"},{"idx":4,"designation":"14","optional":true,"physical":false}],"internal":[{"a":0,"b":1,"kind":{"pass_through":{"opens":true}}},{"a":2,"b":3,"kind":{"pass_through":{}}},{"a":3,"b":4,"kind":"short"}],"flags":{"switches_n_with_l":true}}"#,
    r##"{"$k":"symbol","id":"@SYM_MCB","library_id":"@LIB","item_id":"@ITEM_SYM","version":"1.0.0","sha256":"@HASH","meta":{"key":"ew.iec.mcb-1pn"},"name":{"en-GB":"MCB 1P+N"},"fndef":{"library":"@LIB","item":"@ITEM_FN","version_req":"^1"},"grid":{"num":4000000,"den":1},"graphics":[{"line":{"from":[0,0],"to":[0,8000000],"stroke":{"pen":"p026"}}},{"rect":{"rect":[[-2000000,2000000],[2000000,6000000]],"shadow":2610000,"stroke":{"pen":{"width":183000},"dash":"dash42","colour":{"rgb":"#c0c0c0"}},"fill":"by_layer"}},{"arc":{"rect":[[0,0],[4000000,4000000]],"sweep":90000}},{"text":{"pos":[0,0],"text":"F","align":"centre","angle":90000}}],"cps":[{"idx":0,"fn_cp":0,"pos":[0,0],"dir":"up"},{"idx":1,"fn_cp":1,"pos":[0,8000000],"dir":"down"}],"text_slots":[{"kind":"tag","pos":[4000000,0],"align":"top_left","visible":true},{"kind":{"cp_designation":0},"pos":[1000000,0],"rot":90000,"align":"bottom_left","visible":true,"max_width":10000000}],"arrangements":[{"name":"full","slots":[0,1]}],"anchor":{"slot":"tag"},"iec60617_ref":"S00287"}"##,
    r##"{"$k":"form","id":"@FORM_FRAME","library_id":"@LIB","item_id":"@ITEM_FRAME","version":"1.0.0","sha256":"@HASH","kind":"frame","sheet":{"width":297000000,"height":210000000},"page_to_sheet":{"scale_ppb":986282578,"offset":[9790000,9790000]},"grid":{"header_band":{"num":15875000,"den":3},"columns":{"count":17,"width":{"num":15875000,"den":1}},"rows":{"count":8,"height":{"num":63500000,"den":3}}},"cells":[{"rect":[[200000000,190000000],[280000000,200000000]],"label":{"en-GB":"Project:"},"value":"{project.reference}","style":"text.normal","align":"middle_left","overflow":{"shrink_to_fit":{"min_cap_height":1200000}}}],"data":{"rect":[[16310000,20000000],[277270000,180000000]],"header":{"height":6520000},"rows":{"data":{"height":6520000,"cells":[{"rect":[[16310000,0],[79730000,6520000]],"value":"{row.folio}","style":"text.normal","align":"middle_left","overflow":"ellipsis"}]}},"grid":{"line":{"pen":{"width":183000}},"header_fill":{"rgb":"#c0c0c0"}}},"meta":{"key":"ew.form.qet-a4","licence":"CC-BY-SA-4.0"}}"##,
    r#"{"$k":"part","id":"@PART_NEBU","library_id":"@LIB","item_id":"@ITEM_PSU","version":"2.0.0","sha256":"@HASH","meta":{"status":{"deprecated":{}}},"kind":"cable","manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^1"},"part_number":"NEBU-G78W5-K-2-N-LE5","category":["Cable","Supply","5-core"],"tech_data":[{"key":"length","value":{"value":"2","unit":"m"}}],"free_props":[{"key":"connector","value":"7/8 inch"}],"unit":"m","function_template":[{"fndef":{"library":"@LIB","item":"@ITEM_FN","version_req":"^1"},"main":true,"cps":[0,1]}],"sections":{"cable_cores":[{"core":"BK","colour":["BK"]},{"core":"GNYE","colour":["GN","YE"],"csa":{"awg":18}},{"core":"SH","shield":true,"pair":1}],"plc":{"role":"module","io":[{"dir":"out","bits":8},{"dir":"in","bits":16,"class":"analog"}],"channels":{"count":8,"kind":"do","data_type":"Bool","designation":"{n}"},"bit_of":"n","unbound":"deactivated"}}}"#,
];
