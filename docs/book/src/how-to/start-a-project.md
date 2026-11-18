# Start a project and use templates

This guide shows how to start a new project, what a template is and what it brings to a project, and how to make a
template, change it and stop using it. The words in **bold** are the words of the window.

## Start a new project

1. On the start page press **New project**, or press `Ctrl+N` (**File ▸ New project…**). The **New project** dialog
   opens.
2. Type the project's **Name**. Under it, **Folder** is where the project's folder is made; **Browse…** chooses
   another one. A folder typed without its whole path, such as `Pumps`, is a folder in your projects folder. The line
   under the folder says where the project will be. A folder that is not there yet is made with the project.
3. Under **Start from**, choose what the project starts with. Each line says in words what it brings:
   - **Empty project**: one empty page, with the program's frame and title block and its settings.
   - Each of your templates, with its frame and title block, its check level, its text styles and its sections. The
     template you use for new projects is marked **used for new projects** and is chosen when the dialog opens.
   - **Another project…**: choose an earlier project to start like it, for example a standard cabinet.
4. A template or a project that has pages shows the box **Also copy its pages and the devices on them**. It is off:
   without it the new project starts with one empty page and none of the old pages or devices. Tick it only when you
   want those pages, for example to start from a standard cabinet.
5. Press **Create**. The project is made and opens. While something is missing or wrong (no name, a file where the
   folder would go, a drive that is not there, a project of that name there already) the line over **Create** says
   what; when the folder is to be made, the line says so.

Nothing comes into the new project that the dialog did not name.

## What a template is

A template is a project folder that says how new projects start. It holds:

- the frame and title block, and your company's name and logo in it;
- the check level (see [The check level](#the-check-level));
- the text styles, the wire colours and sizes, the numbering and the cross-reference style;
- the sections and the structure your drawings always have, and the lists and overviews on their own pages;
- the first revision label.

A template can hold pages and devices too, but a new project takes them only when you tick **Also copy its pages and
the devices on them**. A project made from a template keeps what it started with: changing the template later changes
only the projects you make after that.

## Your templates

**Templates…** on the start page, and in the Project tab under **Templates**, lists your templates. Each line shows the
template's name, what it holds and its folder. Click a template to choose it, then:

- **Use for new projects** marks it for the New project dialog; **Do not use** takes the mark off, so new projects
  start empty unless you choose a template.
- **Edit** opens the template to change it (see [Change a template](#change-a-template)).
- **Rename** changes the name the list shows and New project offers: type the name and press **Save name** or
  `Enter`. The folder keeps its own name. A name another template has is refused.
- **Remove from the list** takes it out of the list. Its folder stays on the disk; **Add a folder…** brings it back.

Under the list, **New template…** makes a template for your company's drawings with the wizard, and **Add a folder…**
takes a template somebody gave you (any project folder).

## Make a template for your company's drawings

**New template…** opens the wizard. It has ten steps: the company, the frame and title block, the colour profile, the
wire cross-sections, identifiers and numbering, structure and sections, revisions, languages and formats, reports and checks,
and a review. Each step says what it is and where you can change it later, and its **Result** shows what your choices
make, such as the name of the third contactor on page 102; the **Example** beside it shows a cover page and a schematic
page with your choices. Your choices are kept until you finish: **Close** asks first, and keeps them unless you
choose **Close and drop my answers**; **New template…** then opens the wizard where you left off.

A new template's texts print readably: no style is lower than 1.2 mm. Their heights and typefaces are on
**Text styles…** in the template (see [Text styles](#text-styles)).

The last step lists every choice. **Use this template for new projects** is ticked: the New project dialog then
chooses this template first. Untick it to keep the template in the list without using it yet. The line under it says
where the template is made: a folder named after it in your templates folder (`Templates` in your projects folder),
with a number after the name when that folder is taken. **Choose another folder…** puts it in another folder. **Create
the template** writes it there, the check level included.

## Make a template of a project

When a project is set up the way you like, **Save as template…** (Project tab, **Templates**) makes the next projects
start like it:

1. Type the template's **Name** and the **Folder** it is made in.
2. **Its pages and their devices** is off: the template takes the project's settings, frames and title blocks, text
   styles, sections and structure, and no pages. Tick it to take the pages too.
3. **Use for new projects** is ticked: the New project dialog then chooses this template first.
4. Press **Save as template**. The template is in your list.

## Change a template

In **Templates…** choose the template and press **Edit**. The template opens like a project, with a band across the
window: "You are editing the template …. Projects made from it later start like this." The title bar says
**Template**. Change what you want and save; **Close the template** on the band closes it. A template does not appear
among the recent projects.

## Stop using a template

In **Templates…** choose the template and press **Do not use**: new projects start empty unless you choose a
template in the New project dialog. **Remove from the list** also takes it out of the list; the folder stays on the
disk.

## Text styles

Every text on a drawing prints in a text style: device tags in **Device tag**, wire numbers in **Wire marker**, the
number beside a terminal in **Terminal number**, and so on. **Text styles…** (Project tab, under **Setup**) lists the
project's styles under what they print: Devices, Wires and cables, Terminals, Page, and Cover and sections. Each line
says where the style prints and has:

- **Typeface**: Standard (Noto Sans), Narrow (Noto Sans Condensed), Mono (Noto Sans Mono, every character equally
  wide) or Technical drawing (Routed Gothic, the single-stroke lettering of technical drawings). The typefaces are
  part of the program, so a drawing looks the same on every computer; the fonts installed on your computer are not
  offered. Technical drawing has no bold: with it, **B** is off and the style prints regular.
- **Height**: the height of a capital letter on paper, in millimetres, as drawing standards give a text's size. Type a
  value, or choose one of the standard heights 1.8, 2.5, 3.5, 5 and 7 mm beside it. A height under 1.2 mm is allowed,
  and the line says that it is too small to read on paper; Check project then warns about it too.
- **B**: bold.
- A sample in the style, in proportion to the others, and **Reset**, which brings the style back to what a new
  project has.

A change shows on the open page at once and is one step of **Undo**. **Standard sizes** gives every style the height a
new project has and keeps typefaces and bold; **Reset all** brings back every style. A new project, and every template
the wizard makes, starts readable; the styles are the project's, and a template carries its own to the projects made
from it.

The warning "Wire marker prints 0.94 mm high, too small to read on paper" opens **Text styles** at that style when you
open it in Messages (double-click, or Enter); **Set to 1.2 mm** on the message mends it in one step.

A free text has its own **Style** in its Properties, chosen by name, and its own **Height** for that one text. The
Properties of a text or a device also say which style it prints in, with **Change styles…** beside it.

## The check level

Before drawings go out, the program checks them. The check level says how strict that check is:

- **Pre-delivery**: what must be right before drawings go out, such as an interruption point without its partner on
  another page. A few tidiness checks are left out, such as a structure code with a separator in it.
- **Full**: every check, the tidiness checks too.

While you draw, the Messages panel shows every check whatever the level. A project made with an earlier version may
have the level **Drafting**: it keeps working and checks as **Full** does, and **Check level…** shows it as Full.

The wizard's step **Reports and checks** sets it for a template, and **Create the template** writes it. In a project
(or an open template), **Check level…** in the Project tab under **Setup** changes it. Only a person sets the check
level, in the window: a script or the command line cannot.
