# Print the workshop's lists

The workshop builds the cabinet from five lists beside the drawings: the wire list, the fitter's sheet; the assembly
list of each terminal strip; the cut list of rails, ducts, cables, wires and tubes; the consumables to order; and,
after a release, the rework list. Each is made from the project and follows it when it changes: you type nothing into
a list.

You need a checked schematic. None of the lists needs a panel layout, but without one a wire has a length only where
one is typed, the assembly list names no rail and the cut list holds no rails or ducts
([Route the cabinet's wires](route-wires.md) gives the wires their lengths). The commands are in the Workshop group of
the Output tab.

## The wire list

1. Choose **Output ▸ Workshop ▸ Open wire list**. The first page of the project's wire list opens (907 on the sample): per
   wire its number, its two ends with their pins or clamps, its colour, its size, its length, what goes on each end
   (such as "ferrule 1.5 BK", or "twin 2.5 BU · 101.6" where two wires share a twin ferrule), the marker of each end
   and its pages. A device in the door prints "(door)" after its tag, a cable's core prints its cable and core (such
   as "-101W1 BN") and "cable" for its length, and a wire without a length prints "—".
2. To print the engineering columns as well, choose **Output ▸ List columns…** on the list's page and tick them:
   Potential, Source, Via, Wire part, Length from, and the parts and strip lengths of the ends.
3. A project without a wire list opens **Report block…** on the wire list instead; **Create** makes it.

## The assembly lists, the cut list and the consumables

4. Choose **Output ▸ Workshop ▸ Open assembly lists**. Each strip starts with a line that says where it goes and how long
   it is, such as "-X1011 · on rail 1 from 0 to 48.2 mm · rail cut to 470 mm · 5 terminals · 48.2 mm with its end
   brackets", then every item in the order the fitter mounts it: end bracket, terminals with their part, colour and
   marker, end plate, end bracket; then its bridges.
5. Choose **Output ▸ Workshop ▸ Open cut list**. Under Rails, Ducts, Cables, Wires and Tubes it lists each piece to cut
   with its part or its size and colour, what it is for, the pieces, the piece length and the total. Each piece is
   rounded up to **Cut to** (5 mm unless the project sets another); a wire is as long as the wire list says.
6. Choose **Output ▸ Workshop ▸ Open consumables**: the metres of each wire, the ferrules and lugs per part, and the
   markers per label set, to order beside the parts list.
7. The wire parts, the ferrules, the markers and the glands these lists name come from the **workshop stock**, the
   parts your shop keeps, in the order you prefer them. Choose **Project ▸ Edit stock…**: Parts opens on the
   project's stock, kind by kind. On a stock part's sheet, **Up**, **Down** and **Take out** change it; another wire,
   ferrule, marker or gland part has **Add to the stock**. How, and how to keep the stock for new projects, is in
   [Choose parts](choose-parts.md#the-workshop-stock).
8. The rows of the workshop settings are under **Output ▸ Workshop ▸ Workshop settings**: Cut to, Marker text,
   Ferrules, Two wires in one clamp and the others. A change shows in the lists at once.

## The rework list after a release

9. After a release, a change to the project is work in the built cabinet. Choose **Output ▸ Workshop ▸ Rework
   list…**. It compares the project as it is now with the newest release whose archive is in the folder beside the
   project; **Compare with** chooses another release, and **Archive file…** an archive file.
10. A line counts the work, then one row per thing the fitter does, in the order he does it: Pull, Replace, Move end,
    Re-make end, Check length, Re-mark, Re-label, Remove device, Mount device, Add and the others, each with what
    changes. "Not listed: wires that only moved on a page or took another route, and a new wire number."
11. **Export…** writes the rework list as an Excel file or a CSV; **Labels for these changes…** opens Labels… on the
    labels the list names. A project without a release says "This project has no released revision yet: the rework
    list compares with a release. Release it on the Check tab, or choose an archive file."

## Print the lists, or hand them over as files

12. To print a list, choose **Output ▸ Print…**, set **Range** to **Pages** and type the list's folios, such as
    `907-907.2`; or open the list's page and choose **Current page**. **Export PDF…** writes every page, the lists
    among them.
13. To hand one list over as a file, open its page and choose **Output ▸ Workshop ▸ Export list…** (also in the
    menu of the list's page). It writes the list on show as an Excel file in the project's language, or, for a file
    name that ends in `.csv`, a CSV in English for a program. A size or a length is a number under a heading that
    carries its unit, such as "Length mm". The package holds every workshop list as an Excel file
    ([Export the labels, the drilling drawings and the package](workshop-package.md)), and on the command line
    `ew export wires` and `ew export report` write one list
    ([Use the command line on the sample project](headless-workflow.md)).

What each command does is in the [command bar reference](../reference/window/command-bar.md#output).
