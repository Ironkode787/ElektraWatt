// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `draw`. The tools: select, place, wire, route, interruption points, black boxes, texts, strips, bonds, the panel layout.
//!
//! The sections follow one another in the order of `super::SECTIONS`, and that order is the registry's: it sets the
//! order of menu items, palette rows and the shortcut list. Move an entry only where its menu neighbours are.

use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    keeps_banner(placed(
        place(Group::Select, 1, CommandSize::Large, None),
        "pointer",
        act("tool.select", View, Menu::Draw, &[], has_pages, None, |s, _| {
            s.act_tool("select")
        }),
    )),
    keeps_banner(placed(
        place(Group::Annotate, 2, CommandSize::Small, None),
        "ruler-measure",
        act("tool.measure", View, Menu::Draw, &[], has_pages, None, |s, _| {
            s.act_tool("measure")
        }),
    )),
    // Place and arrange: the Insert menu, the transform keys, Move, Delete, Change symbol.
    keeps_banner(in_canvas(placed(
        place(Group::Place, 1, CommandSize::Large, None),
        "circuit-switch-open",
        act(
            "tool.place",
            Command,
            Menu::Draw,
            &["I"],
            editable_page,
            None,
            |s, _| s.act_open_picker(crate::tools::PickerMode::Place),
        ),
    ))),
    keeps_banner(in_canvas(placed(
        place(Group::Place, 2, CommandSize::Large, Some(key!("bar-device"))),
        "package",
        act(
            "tool.place_part",
            Command,
            Menu::Draw,
            &["Shift+I"],
            editable_page,
            None,
            |s, _| s.act_open_picker(crate::tools::PickerMode::Parts),
        ),
    ))),
    // Draw ▸ Place ▸ Terminal…: its own small form (strip, kind, how many, from which number; FT-1695), then the Place
    // tool floating the first terminal; a click lays the row. Terminals are placed this way only.
    keeps_banner(with_params(
        "strip?:text, kind?:text, count?:uint, from?:text",
        in_canvas(placed(
            place(Group::Place, 3, CommandSize::Small, Some(key!("bar-terminal"))),
            "ew-terminal-strip",
            with_aliases(
                &[("strip", ParamForm::Whole)],
                act(
                    "tool.place_terminal",
                    Command,
                    Menu::Draw,
                    &[],
                    editable_page,
                    None,
                    |s, p| s.act_place_terminal(p),
                ),
            ),
        )),
    )),
    keeps_banner(with_params(
        "letter?:text, tag?:text, dialog?:bool",
        act(
            "strip.new",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            grp(Group::Place),
            |s, p| s.act_strip_new(p),
        ),
    )),
    // The structure node and the external reference, one previewed command each.
    with_params(
        "kind:enum(functional_assignment|function|installation_site|location|document_kind|user_defined|type), designation:text, parent?:text",
        placed(
            place(Group::Project, 2, CommandSize::Small, Some(key!("bar-structure"))),
            "sitemap",
            with_aliases(
                &[("parent", ParamForm::Whole)],
                act(
                    "structure.new",
                    Command,
                    Menu::Project,
                    &[],
                    writable,
                    None,
                    |s, p| s.act_structure_new(p),
                ),
            ),
        ),
    ),
    with_params(
        "location:text, device:text, connection?:text, project?:text",
        with_aliases(
            &[("location", ParamForm::Whole)],
            act(
                "external_ref.new",
                Command,
                Menu::Project,
                &[],
                writable,
                grp(Group::Project),
                |s, p| s.act_external_ref_new(p),
            ),
        ),
    ),
    keeps_banner(with_params(
        "symbol?:text, part?:text, function?:ref, device?:ref, template?:uint, letter?:text, strip?:text, terminal_designation?:text, designation?:text, count?:uint",
        hidden(in_canvas(with_aliases(
            &[
                ("symbol", ParamForm::Whole),
                ("part", ParamForm::Whole),
                ("function", ParamForm::Whole),
                ("device", ParamForm::Whole),
                ("strip", ParamForm::Whole),
            ],
            act(
                "place.pick",
                Command,
                Menu::Draw,
                &[],
                editable_page,
                None,
                |s, p| s.act_place_pick(p),
            ),
        ))),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(act(
            "picker.filter",
            View,
            Menu::Draw,
            &[],
            picker_open,
            None,
            |s, p| s.act_picker_filter(p),
        )),
    )),
    keeps_banner(with_params(
        "step?:int, to?:enum(first|last)",
        hidden(act(
            "picker.row",
            View,
            Menu::Draw,
            &[],
            picker_open,
            None,
            |s, p| s.act_picker_row(p),
        )),
    )),
    // The browser's tree node, and a part opened to its functions or closed (Left and Right).
    keeps_banner(with_params(
        "to?:text, step?:int",
        hidden(act(
            "picker.category",
            View,
            Menu::Draw,
            &[],
            picker_open,
            None,
            |s, p| s.act_picker_category(p),
        )),
    )),
    keeps_banner(with_params(
        "open?:bool",
        hidden(act(
            "picker.open",
            View,
            Menu::Draw,
            &[],
            picker_open,
            None,
            |s, p| s.act_picker_open(p),
        )),
    )),
    keeps_banner(hidden(with_params(
        // `arm` reads the optional device letter and, for a terminal, the strip and its designation
        // (`terminal_designation`; keeps `designation` as its old name).
        "letter?:text, strip?:text, terminal_designation?:text, designation?:text",
        act(
            "picker.choose",
            Command,
            Menu::Draw,
            &[],
            picker_open,
            None,
            |s, p| s.act_picker_choose(p),
        ),
    ))),
    // The current row, a symbol on another connection grid, placed on a new page with its grid.
    keeps_banner(hidden(act(
        "picker.new_page",
        Command,
        Menu::Draw,
        &[],
        picker_open,
        None,
        |s, _| s.act_picker_new_page(),
    ))),
    // J opens the Place tool's Join chooser (doc 16a §6.22) while the armed symbol can be an
    // auxiliary function; the chooser's own keys are its actions.
    keeps_banner(hidden(in_canvas(act(
        "place.join",
        Command,
        Menu::Draw,
        &["J"],
        place_joinable,
        None,
        |s, _| s.act_place_join(),
    )))),
    keeps_banner(with_params(
        "step?:int, to?:enum(first|last)",
        hidden(act(
            "chooser.row",
            View,
            Menu::Draw,
            &[],
            chooser_open,
            None,
            |s, p| s.act_chooser_row(p),
        )),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(act(
            "chooser.type",
            View,
            Menu::Draw,
            &[],
            chooser_open,
            None,
            |s, p| s.act_chooser_type(p),
        )),
    )),
    keeps_banner(with_params(
        "row?:uint",
        hidden(act(
            "chooser.choose",
            Command,
            Menu::Draw,
            &[],
            chooser_open,
            None,
            |s, p| s.act_chooser_choose(p),
        )),
    )),
    keeps_banner(in_canvas(placed(
        place(Group::Arrange, 1, CommandSize::Small, Some(key!("bar-rotate"))),
        "rotate-clockwise",
        act(
            "tool.rotate_cw",
            Command,
            Menu::Draw,
            &["R"],
            editable_page,
            None,
            |s, _| s.act_tool_key(crate::tools::ToolKey::RotateCw),
        ),
    ))),
    keeps_banner(in_canvas(act(
        "tool.rotate_ccw",
        Command,
        Menu::Draw,
        &["Shift+R"],
        editable_page,
        grp(Group::Arrange),
        |s, _| s.act_tool_key(crate::tools::ToolKey::RotateCcw),
    ))),
    keeps_banner(in_canvas(placed(
        place(Group::Arrange, 2, CommandSize::Small, Some(key!("bar-mirror"))),
        "flip-vertical",
        act(
            "tool.mirror_x",
            Command,
            Menu::Draw,
            &["X"],
            editable_page,
            None,
            |s, _| s.act_tool_key(crate::tools::ToolKey::MirrorX),
        ),
    ))),
    keeps_banner(in_canvas(act(
        "tool.mirror_y",
        Command,
        Menu::Draw,
        &["Y"],
        editable_page,
        grp(Group::Arrange),
        |s, _| s.act_tool_key(crate::tools::ToolKey::MirrorY),
    ))),
    keeps_banner(in_canvas(act(
        "tool.next_variant",
        Command,
        Menu::Draw,
        &["V"],
        editable_page,
        grp(Group::Arrange),
        |s, _| s.act_tool_key(crate::tools::ToolKey::NextVariant),
    ))),
    keeps_banner(in_canvas(act(
        "tool.next_rep_type",
        View,
        Menu::Draw,
        &["Shift+V"],
        editable_page,
        grp(Group::Arrange),
        |s, _| s.act_tool_key(crate::tools::ToolKey::NextRepType),
    ))),
    keeps_banner(in_canvas(act(
        "tool.move",
        Command,
        Menu::Draw,
        &["M"],
        editable_page,
        grp(Group::Arrange),
        |s, _| s.act_tool_move(),
    ))),
    keeps_banner(in_canvas(act(
        "edit.delete_selection",
        Command,
        Menu::Edit,
        &["Delete"],
        editable_selection,
        slot_sep(Menu::Edit),
        |s, _| s.act_delete_selection(),
    ))),
    // The clipboard (§8.8). The chords are the canvas's: a text field of the property grid or the find
    // pane keeps egui's own copy and paste, and the window routes no chord while a text field has the keyboard.
    in_canvas(act(
        "edit.cut",
        Command,
        Menu::Edit,
        &["Ctrl+X"],
        selection_editable,
        slot(Menu::Edit),
        |s, _| s.act_cut(),
    )),
    in_canvas(act(
        "edit.copy",
        View,
        Menu::Edit,
        &["Ctrl+C"],
        selection_shown,
        slot(Menu::Edit),
        |s, _| s.act_copy(),
    )),
    with_params(
        "text?:text",
        in_canvas(with_icon(
            "clipboard-list",
            act(
                "edit.paste_clip",
                Command,
                Menu::Edit,
                &["Ctrl+V"],
                paste_page,
                slot(Menu::Edit),
                |s, p| s.act_paste_clip(p),
            ),
        )),
    ),
    in_canvas(act(
        "edit.duplicate",
        Command,
        Menu::Edit,
        &["Ctrl+D"],
        selection_editable,
        slot(Menu::Edit),
        |s, _| s.act_duplicate(),
    )),
    in_canvas(act(
        "edit.copy_tags",
        View,
        Menu::Edit,
        &[],
        selection_shown,
        slot(Menu::Edit),
        |s, _| s.act_copy_tags(),
    )),
    in_canvas(act(
        "edit.select_all",
        View,
        Menu::Edit,
        &["Ctrl+A"],
        page_shown,
        slot(Menu::Edit),
        |s, _| s.act_select_all(),
    )),
    with_params(
        "text:text",
        headless(act(
            "clipboard.text",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_clipboard_text(p),
        )),
    ),
    keeps_banner(with_params(
        "symbol?:text",
        in_canvas(with_icon(
            "circuit-switch-open",
            with_aliases(
                &[("symbol", ParamForm::Whole)],
                act(
                    "rep.exchange",
                    Command,
                    Menu::Draw,
                    &[],
                    editable_page,
                    grp(Group::Arrange),
                    |s, p| s.act_rep_exchange(p),
                ),
            ),
        )),
    )),
    keeps_banner(hidden(in_canvas(act(
        "tool.finish",
        View,
        Menu::Edit,
        &["Shift+Enter"],
        has_pages,
        None,
        |s, p| s.act_tool_finish(p),
    )))),
    keeps_banner(with_params(
        "text:text",
        hidden(in_canvas(act(
            "text.input",
            View,
            Menu::Edit,
            &[],
            has_pages,
            None,
            |s, p| s.act_text_input(p),
        ))),
    )),
    // The Wire tool, the proposals' Tab, route edit, the junction sequence editor and
    // the crossings setting. Begin.
    keeps_banner(in_canvas(placed(
        place(Group::Connect, 1, CommandSize::Large, Some(key!("bar-wire"))),
        "route-2",
        act(
            "tool.wire",
            Command,
            Menu::Draw,
            &["W"],
            wire_page,
            None,
            |s, _| s.act_tool_wire(),
        ),
    ))),
    // Ctrl+Enter: a click with Alt held at the grid cursor (a placement without its autoconnect proposals, a bend
    // without object snaps), the keyboard's twin of `canvas.activate {alt}` (per-context chords).
    keeps_banner(hidden(in_canvas(act(
        "canvas.activate_alt",
        View,
        Menu::Page,
        &["Ctrl+Enter"],
        has_pages,
        None,
        |s, p| s.act_canvas_activate_alt(p),
    )))),
    keeps_banner(hidden(in_canvas(act(
        "tool.next_proposal",
        View,
        Menu::Edit,
        &["Tab"],
        editable_page,
        None,
        |s, _| s.send_proposal_key(),
    )))),
    // Placing on a panel page (doc 12 §12.4). Tab, R, X and the arrows keep their bindings:
    // on a panel page the shell maps them to these hidden actions (`ShellModel::shortcut_action`); Shift+Tab is
    // `layout.previous`'s own canvas binding.
    with_params(
        "device:ref",
        hidden(act(
            "tool.place_layout",
            Command,
            Menu::Draw,
            &[],
            panel_page,
            None,
            |s, p| s.act_place_layout(p),
        )),
    ),
    hidden(in_canvas(act(
        "layout.next",
        View,
        Menu::Edit,
        &[],
        panel_page,
        None,
        |s, _| s.act_layout_step(1),
    ))),
    hidden(in_canvas(act(
        "layout.previous",
        View,
        Menu::Edit,
        &["Shift+Tab"],
        panel_page,
        None,
        |s, _| s.act_layout_step(-1),
    ))),
    with_params(
        "dx?:int, dy?:int",
        hidden(in_canvas(act(
            "layout.nudge",
            Command,
            Menu::Edit,
            &[],
            // A selected placement, or a floating layout ghost (the shell refuses without either).
            panel_page,
            None,
            |s, p| s.act_layout_nudge(p),
        ))),
    ),
    hidden(in_canvas(act(
        "layout.rotate",
        Command,
        Menu::Edit,
        &[],
        panel_page,
        None,
        |s, _| s.act_layout_turn(crate::tools::ToolKey::RotateCw),
    ))),
    hidden(in_canvas(act(
        "layout.flip",
        Command,
        Menu::Edit,
        &[],
        panel_page,
        None,
        |s, _| s.act_layout_turn(crate::tools::ToolKey::MirrorX),
    ))),
    hidden(in_canvas(act(
        "layout.lock_selection",
        Command,
        Menu::Edit,
        &[],
        panel_selection,
        None,
        |s, _| s.act_layout_lock(true),
    ))),
    hidden(in_canvas(act(
        "layout.unlock_selection",
        Command,
        Menu::Edit,
        &[],
        panel_selection,
        None,
        |s, _| s.act_layout_lock(false),
    ))),
    with_params(
        "mode?:enum(free|push|swap), grid?:uint",
        hidden(in_canvas(act(
            "layout.options",
            View,
            Menu::Edit,
            &[],
            panel_page,
            None,
            |s, p| s.act_layout_options(p),
        ))),
    ),
    // Draw ▸ Layout (Rail, Duct, Auto-fill rail…) on a panel layout page with a
    // surface view, the draw tools' options card, and Project ▸ Enclosure….
    keeps_banner(in_canvas(placed(
        place(Group::PanelLayout, 1, CommandSize::Small, Some(key!("bar-rail"))),
        "ew-din-rail",
        act(
            "tool.rail_draw",
            Command,
            Menu::Draw,
            &[],
            panel_view,
            None,
            |s, _| s.act_tool_rail_draw(),
        ),
    ))),
    keeps_banner(in_canvas(placed(
        place(Group::PanelLayout, 2, CommandSize::Small, Some(key!("bar-duct"))),
        "ew-duct",
        act(
            "tool.duct_draw",
            Command,
            Menu::Draw,
            &[],
            panel_view,
            None,
            |s, _| s.act_tool_duct_draw(),
        ),
    ))),
    with_params(
        "rail?:text, order?:enum(schematic|dt)",
        placed(
            place(
                Group::PanelLayout,
                3,
                CommandSize::Small,
                Some(key!("bar-auto-fill")),
            ),
            "wand",
            act(
                "layout.auto_fill_rail",
                Command,
                Menu::Draw,
                &[],
                panel_view,
                None,
                |s, p| s.act_layout_auto_fill_rail(p),
            ),
        ),
    ),
    with_params(
        "part?:text, width?:text, height?:text",
        hidden(act(
            "layout.draw_options",
            View,
            Menu::Draw,
            &[],
            panel_view,
            None,
            |s, p| s.act_layout_draw_options(p),
        )),
    ),
    with_params(
        "location?:text, part?:text",
        placed(
            place(Group::Project, 3, CommandSize::Small, Some(key!("bar-enclosure"))),
            "door",
            act(
                "layout.enclosure_new",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_layout_enclosure_new(p),
            ),
        ),
    ),
    keeps_banner(hidden(in_canvas(act(
        "tool.connect_on_move",
        Command,
        Menu::Edit,
        &["C"],
        editable_page,
        None,
        |s, p| s.act_tool_connect_on_move(p),
    )))),
    // Edit route on the Draw tab's Connect group, beside Wire (the polyline: Wire has the route icon).
    keeps_banner(in_canvas(placed(
        place(Group::Connect, 2, CommandSize::Small, None),
        "ew-shape-polyline",
        act(
            "tool.route_edit",
            Command,
            Menu::Draw,
            &[],
            one_connection,
            grp(Group::Connect),
            |s, _| s.act_route_edit(),
        ),
    ))),
    // Route page and Route selection after Edit route, previewed (no default key); after Interruption,
    // so the two stack in one column of their own. Each its own icon, neither the generated badge's
    // (C1): the page as a whole, and a pull on the selected wires.
    keeps_banner(placed(
        place(Group::Connect, 7, CommandSize::Small, None),
        "file-settings",
        act(
            "route.page",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            grp(Group::Connect),
            |s, _| s.act_route_page(),
        ),
    )),
    keeps_banner(placed(
        place(Group::Connect, 8, CommandSize::Small, None),
        "magnet",
        act(
            "route.selection",
            Command,
            Menu::Draw,
            &[],
            route_selection,
            grp(Group::Connect),
            |s, _| s.act_route_selection(),
        ),
    )),
    keeps_banner(in_canvas(act(
        "junction.open",
        View,
        Menu::Draw,
        &[],
        one_junction,
        grp(Group::Connect),
        |s, _| s.act_junction_open(),
    ))),
    keeps_banner(with_params(
        "from:uint, to:uint",
        hidden(act(
            "junction.member",
            Command,
            Menu::Edit,
            &[],
            junction_editor_open,
            None,
            |s, p| s.act_junction_member(p),
        )),
    )),
    keeps_banner(with_params(
        "style:enum(dot|bevel), orient?:uint",
        hidden(act(
            "junction.style",
            Command,
            Menu::Edit,
            &[],
            junction_editor_open,
            None,
            |s, p| s.act_junction_style(p),
        )),
    )),
    keeps_banner(with_params(
        "wiring:enum(star|chain|style_default)",
        hidden(act(
            "junction.wiring",
            Command,
            Menu::Edit,
            &[],
            junction_editor_open,
            None,
            |s, p| s.act_junction_wiring(p),
        )),
    )),
    keeps_banner(with_params(
        "on?:bool",
        act(
            "project.crossings",
            Command,
            Menu::Draw,
            &[],
            writable,
            grp(Group::Connect),
            |s, p| s.act_crossings(p),
        ),
    )),
    // Draw ▸ Connect ▸ Print wire numbers (FT-1692), beside the crossings and the wire markers: every wire's number
    // on the page, the PDF and print.
    keeps_banner(with_params(
        "on?:bool",
        act(
            "project.wire_numbers",
            Command,
            Menu::Draw,
            &[],
            writable,
            grp(Group::Connect),
            |s, p| s.act_wire_numbers(p),
        ),
    )),
    // End.
    // Interruption points and black boxes.
    keeps_banner(in_canvas(placed(
        place(
            Group::Connect,
            6,
            CommandSize::Small,
            Some(key!("bar-interruption")),
        ),
        "ew-interruption-point",
        act(
            "tool.ip",
            Command,
            Menu::Draw,
            &["P"],
            editable_page,
            None,
            |s, _| s.act_tool_ip(),
        ),
    ))),
    keeps_banner(with_params(
        "symbol?:text, part?:text, template?:uint, letter?:text",
        in_canvas(placed(
            place(Group::Place, 4, CommandSize::Small, None),
            "ew-black-box",
            with_aliases(
                &[("symbol", ParamForm::Whole), ("part", ParamForm::Whole)],
                act(
                    "tool.black_box",
                    Command,
                    Menu::Draw,
                    &["B"],
                    editable_page,
                    None,
                    |s, p| s.act_tool_black_box(p),
                ),
            ),
        )),
    )),
    keeps_banner(with_params(
        "direction?:enum(source|target|undirected)",
        in_canvas(act(
            "tool.ip_direction",
            View,
            Menu::Draw,
            &["D"],
            editable_page,
            grp(Group::Connect),
            |s, p| s.act_ip_direction(p),
        )),
    )),
    keeps_banner(in_canvas(act(
        "tool.star_source",
        View,
        Menu::Draw,
        &[],
        editable_page,
        grp(Group::Connect),
        |s, _| s.act_ip_key(crate::tools::ToolKey::ToggleStar),
    ))),
    keeps_banner(with_params(
        "text:text",
        hidden(in_canvas(act(
            "ip.name",
            View,
            Menu::Edit,
            &[],
            editable_page,
            None,
            |s, p| s.act_ip_name(p),
        ))),
    )),
    // `external`: an external reference's ID, alias or printed text (`JE-N1000-03:L`); empty clears it.
    keeps_banner(with_params(
        "external:text",
        hidden(in_canvas(with_aliases(
            &[("external", ParamForm::Whole)],
            act(
                "ip.external",
                View,
                Menu::Edit,
                &[],
                editable_page,
                None,
                |s, p| s.act_ip_external(p),
            ),
        ))),
    )),
    keeps_banner(with_params(
        "folio?:text, x_mm?:mm, y_mm?:mm",
        in_canvas(act(
            "ip.continue_on",
            Command,
            Menu::Draw,
            &[],
            one_ip,
            grp(Group::Connect),
            |s, p| s.act_ip_continue_on(p),
        )),
    )),
    keeps_banner(with_params(
        "by?:enum(name|potential), chain_order?:bool",
        in_canvas(act(
            "ip.arrange_selected",
            Command,
            Menu::Draw,
            &[],
            two_ips,
            grp(Group::Connect),
            |s, p| s.act_ip_arrange(p),
        )),
    )),
    keeps_banner(with_params(
        "grip:uint",
        hidden(in_canvas(act(
            "box.grip",
            Command,
            Menu::Edit,
            &[],
            one_box,
            None,
            |s, p| s.act_box_grip(p),
        ))),
    )),
    keeps_banner(in_canvas(act(
        "box.add_cp",
        Command,
        Menu::Draw,
        &[],
        one_box,
        grp(Group::Place),
        |s, _| s.act_box_add_cp(),
    ))),
    // End.
    // Text and graphic tools.
    keeps_banner(in_canvas(placed(
        place(Group::Annotate, 1, CommandSize::Large, None),
        "typography",
        act(
            "tool.text",
            Command,
            Menu::Draw,
            &["T"],
            editable_page,
            None,
            |s, _| s.act_tool_text(),
        ),
    ))),
    keeps_banner(with_params(
        "shape?:enum(line|polyline|rect|circle|arc|polygon)",
        in_canvas(with_icon(
            "pencil",
            act(
                "tool.graphic",
                Command,
                Menu::Draw,
                &["L"],
                editable_page,
                grp_sub(Group::Annotate, Submenu::Graphic),
                |s, p| s.act_tool_graphic(p),
            ),
        )),
    )),
    keeps_banner(with_params(
        "bind:text",
        hidden(in_canvas(act(
            "text.bind",
            View,
            Menu::Edit,
            &[],
            editable_page,
            None,
            |s, p| s.act_text_bind(p),
        ))),
    )),
    keeps_banner(in_canvas(with_icon(
        "pencil",
        act(
            "text.edit",
            Command,
            Menu::Draw,
            &["F2"],
            one_free_text,
            grp(Group::Annotate),
            |s, _| s.act_text_edit(),
        ),
    ))),
    // Edit ▸ Bridge terminals.
    keeps_banner(in_canvas(act(
        "strip.bridge",
        Command,
        Menu::Draw,
        &[],
        two_strip_terminals,
        grp(Group::Connect),
        |s, _| s.act_strip_bridge(),
    ))),
    // The strip editor (§6.9; 262 → 268).
    with_params(
        "strip?:text",
        act(
            "strip.edit",
            View,
            Menu::Edit,
            &[],
            project_open,
            slot(Menu::Edit),
            |s, p| s.act_strip_edit(p),
        ),
    ),
    with_params(
        "count?:uint, designation?:text",
        in_navigator(act(
            "strip.insert",
            Command,
            Menu::Edit,
            &["Insert"],
            strips_editor,
            None,
            |s, p| s.act_strip_insert(p),
        )),
    ),
    in_navigator(act(
        "strip.delete_rows",
        Command,
        Menu::Edit,
        &["Ctrl+Delete"],
        strips_editor,
        None,
        |s, _| s.act_strip_delete_rows(),
    )),
    // Alt+Arrow Up and Alt+Arrow Down stay bound to `pages.move_up` and `pages.move_down` (the keymap test forbids a
    // second binding); while the Terminal strips navigator has the focus they run this action with `step` -1 or 1
    // (`ShellModel::shortcut_action`).
    with_params(
        "step?:int, before?:text",
        in_navigator(act(
            "strip.move",
            Command,
            Menu::Edit,
            &[],
            strips_editor,
            None,
            |s, p| s.act_strip_move(p),
        )),
    ),
    with_params(
        "kind?:enum(end_plate|end_clamp|partition|marker|cover), part?:text, before?:bool",
        in_navigator(act(
            "strip.add_accessory",
            Command,
            Menu::Edit,
            &[],
            strips_editor,
            slot(Menu::Edit),
            |s, p| s.act_strip_add_accessory(p),
        )),
    ),
    in_navigator(act(
        "strip.unbridge",
        Command,
        Menu::Edit,
        &[],
        strips_editor,
        None,
        |s, _| s.act_strip_unbridge(),
    )),
    // Swap two cores of one cable (FR-CBL-016). The navigators have no row menu yet, so it sits
    // in the Edit menu with the strip editor's gestures.
    in_navigator(act(
        "cable.swap_cores",
        Command,
        Menu::Edit,
        &[],
        cables_editor,
        slot(Menu::Edit),
        |s, _| s.act_cable_swap_cores(),
    )),
    // FT-490's generic half: Delete and F2 in a navigator tree (`table.clear` and `table.edit` in tree mode run these,
    // `ShellModel::shortcut_action`).
    with_params(
        "kind?:text",
        hidden(in_navigator(act(
            "navigator.delete",
            Command,
            Menu::Edit,
            &[],
            writable,
            None,
            |s, p| s.act_navigator_delete(p),
        ))),
    ),
    with_params(
        "kind?:text, text?:text, cancel?:bool",
        hidden(in_navigator(act(
            "navigator.rename",
            Command,
            Menu::Edit,
            &[],
            writable,
            None,
            |s, p| s.act_navigator_rename(p),
        ))),
    ),
    // G-PP triage row 7: Draw ▸ Bond, `bond.create` on the selected connection or two named CPs, previewed.
    keeps_banner(with_params(
        "kind?:enum(pelv_0v_pe|fe_pe|equipotential|shield), a?:text, b?:text",
        in_canvas(act(
            "bond.draw",
            Command,
            Menu::Draw,
            &[],
            bond_ends,
            grp_sub(Group::Connect, Submenu::Bond),
            |s, p| s.act_bond_draw(p),
        )),
    )),
    // Draw ▸ Define potential…, beside Bond: `potential.define` at the selection's point, through
    // its parameter form.
    keeps_banner(with_params(
        "name?:text, type?:text, voltage?:text",
        in_canvas(act(
            "potential.draw",
            Command,
            Menu::Draw,
            &[],
            potential_point,
            grp(Group::Connect),
            |s, p| s.act_potential_draw(p),
        )),
    )),
    // Draw ▸ Connect ▸ New cable… (`cable.create` from a cable part, with its cores) and Connect
    // cores… (the selected wires become the cable's cores, by order), each through its form.
    with_params(
        "part:text",
        act(
            "cable.new",
            Command,
            Menu::Draw,
            &[],
            writable,
            grp(Group::Connect),
            |s, p| s.act_cable_new(p),
        ),
    ),
    with_params(
        "cable:text",
        in_canvas(act(
            "cable.connect_cores",
            Command,
            Menu::Draw,
            &[],
            selection_editable,
            grp(Group::Connect),
            |s, p| s.act_cable_connect_cores(p),
        )),
    ),
    // Draw ▸ Connect ▸ Cable… (FT-1685): the selected wires as a cable's cores in one form, on the bar after Route
    // selection, in a column of its own.
    with_params(
        "tag?:text, type?:text, cores?:json",
        in_canvas(placed(
            place(Group::Connect, 9, CommandSize::Small, Some(key!("bar-cable"))),
            "ew-cable",
            act(
                "cable.from_wires",
                Command,
                Menu::Draw,
                &[],
                selection_editable,
                grp(Group::Connect),
                |s, p| s.act_cable_from_wires(p),
            ),
        )),
    ),
    // Define potential… from a Potentials row's context menu, at the row's point.
    keeps_banner(with_params(
        "row:text",
        hidden(in_navigator(act(
            "potential.draw_row",
            Command,
            Menu::Draw,
            &[],
            writable,
            None,
            |s, p| s.act_potential_draw_row(p),
        ))),
    )),
    // FT-1700: Accept and Decline on the card of a potential reasoned for the selected wire.
    keeps_banner(with_params(
        "accept:bool",
        hidden(in_canvas(act(
            "potential.offer",
            Command,
            Menu::Draw,
            &[],
            writable,
            None,
            |s, p| s.act_potential_offer(p),
        ))),
    )),
    // The Wire marker and Structure box tools, their fields and the marker mode.
    keeps_banner(in_canvas(placed(
        // "Marker" on the bar: no wider than Measure above it, so the group keeps its width.
        place(Group::Annotate, 3, CommandSize::Small, Some(key!("bar-marker"))),
        "tag",
        act(
            "tool.marker",
            Command,
            Menu::Draw,
            &["Shift+W"],
            editable_page,
            None,
            |s, _| s.act_tool_marker(),
        ),
    ))),
    keeps_banner(with_params(
        "csa?:text, colours?:text",
        hidden(in_canvas(act(
            "marker.fields",
            View,
            Menu::Draw,
            &[],
            editable_page,
            None,
            |s, p| s.act_marker_fields(p),
        ))),
    )),
    keeps_banner(in_canvas(with_icon(
        "ew-structure-box",
        act(
            "tool.structure_box",
            Command,
            Menu::Draw,
            &["Shift+B"],
            editable_page,
            grp(Group::Annotate),
            |s, _| s.act_tool_structure_box(),
        ),
    ))),
    keeps_banner(with_params(
        "style?:enum(dotted|dash_dot), caption?:text, location?:text, external?:text",
        hidden(in_canvas(with_aliases(
            &[("location", ParamForm::Whole), ("external", ParamForm::Whole)],
            act(
                "structure_box.fields",
                View,
                Menu::Draw,
                &[],
                editable_page,
                None,
                |s, p| s.act_structure_box_fields(p),
            ),
        ))),
    )),
    keeps_banner(with_params(
        "mode:enum(per_wire|all|deviations|explicit|none)",
        act(
            "project.marker_mode",
            Command,
            Menu::Draw,
            &[],
            writable,
            grp_sub(Group::Connect, Submenu::WireMarkers),
            |s, p| s.act_marker_mode(p),
        ),
    )),
    // End.
    // Images.
    keeps_banner(in_canvas(with_icon(
        "file",
        act(
            "tool.image",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            grp(Group::Annotate),
            |s, _| s.act_tool_image(),
        ),
    ))),
    keeps_banner(with_params(
        "asset:text",
        hidden(in_canvas(act(
            "image.pick",
            View,
            Menu::Draw,
            &[],
            editable_page,
            None,
            |s, p| s.act_image_pick(p),
        ))),
    )),
    keeps_banner(with_params(
        "path:path",
        hidden(in_canvas(act(
            "image.file",
            View,
            Menu::Draw,
            &[],
            editable_page,
            None,
            |s, p| s.act_image_file(p),
        ))),
    )),
    placed(
        place(
            Group::Standards,
            2,
            CommandSize::Small,
            Some(key!("bar-company-standard")),
        ),
        "building-factory-2",
        act(
            "tools.company_standard",
            Command,
            Menu::Project,
            &[],
            always,
            None,
            |s, _| s.act_wizard_open(),
        ),
    ),
    // Tools ▸ Run checks.
    placed(
        place(Group::Run, 1, CommandSize::Large, Some(key!("bar-check-project"))),
        "list-check",
        act(
            "messages.check",
            Command,
            Menu::Check,
            &["F7"],
            project_open,
            None,
            |s, _| s.act_messages_check(),
        ),
    ),
    // Tools, Renumber devices… and Purge unused images…
    // Ctrl+Shift+N, doc 16 §4.4's shell binding.
    with_params(
        "scope?:enum(project|page|selection), scheme?:text, include_frozen?:bool, only_nonconforming?:bool",
        placed(
            place(Group::Arrange, 4, CommandSize::Small, Some(key!("bar-renumber"))),
            "sort-ascending-numbers",
            act(
                "devices.renumber",
                Command,
                Menu::Draw,
                &["Ctrl+Shift+N"],
                writable,
                None,
                |s, p| s.act_devices_renumber(p),
            ),
        ),
    ),
    with_params(
        "confirm?:bool",
        act(
            "project.purge_assets",
            Command,
            Menu::Project,
            &[],
            writable,
            slot_sep(Menu::Project),
            |s, p| s.act_purge_assets(p),
        ),
    ),
];
