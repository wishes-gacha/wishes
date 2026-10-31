# Wishes Core Concepts

> [!WARNING]
>
> This document was translated using AI tools and may be inaccurate.
>
> For the most accurate information, please refer to [*concepts.md*](concepts.md) (*Simplified Chinese* version)

> [!NOTE]
>
> This document is intended for users who wish to **understand the internal model of `Wishes`** or **create custom banners**.
>
> No programming background is required — this document describes how `Wishes` works in plain language.
>
> This document was generated with AI tools and reviewed by a human.

The flexibility of `Wishes` comes from a small set of general-purpose abstractions.
Once you understand the concepts in this document, you will be able to **build any banner from scratch** — whether you are simulating the complete gacha mechanics of a specific game, or constructing a custom random system.

## 1. Design Philosophy

The design of `Wishes` revolves around three core ideas:

**First, everything is a tag.**
In most gacha tools, attributes such as "rarity", "type", and "which game it belongs to" tend to be hard-coded into the program. `Wishes` instead abstracts **all descriptive information** into a unified concept called a "tag" — whether it describes card attributes, banner membership, or the grouping of a wish result, everything is expressed the same way. This allows the system to adapt to any game without writing separate code for each one.

**Second, logic and data are separated.**
`Wishes` splits "wishing" into two independent questions:

- **Wish logic** is only responsible for answering: "What type of card should this draw hit?"
- **Decks** are only responsible for answering: "Which cards match this type?"

The two communicate through the intermediate layer of **tags**, and neither knows the other's concrete implementation. The benefits are:

- **Logic is reusable** — the same wish logic can drive multiple banners.
- **Data is extensible** — adding a new game or a new banner usually only requires adding data files, with no code changes.

**Third, data is read-only and forkable.**
Official data and pack data are **always read-only** at runtime. Any edit a user makes to them is never applied in place; instead, a **local copy is forked**.
As a result, the integrity of official data, the composability of packs, and the customizability of user data can all coexist — the three sources never conflict.

## 2. Tags: A Unified Way to Describe Things

### 2.1 Regular Tags

A **tag** consists of two parts:

- **Namespace** — describes "what this is about", such as `rarity`, `type`, or `game`.
- **Value** — describes "what it specifically is", such as `5`, `character`, or `genshin`.

When written together, they are separated by a colon, for example `rarity:5`, `type:character`, or `game:genshin`.

Two tags are equal only when **both the namespace and the value are identical**. This means `type:character` and `rarity:character` are two different tags — even though their values are both `character`, they describe different aspects of an object.

`Wishes` includes three common namespaces out of the box:

| Namespace | Purpose | Example |
| :--- | :--- | :--- |
| `game` | Game name | `game:genshin` |
| `type` | Object type | `type:character`, `type:weapon` |
| `rarity` | Rarity | `rarity:5`, `rarity:4` |

Of course, you are free to use other namespaces as well, such as `element:pyro` or `path:destruction`.

### 2.2 Event Tags

**Event tags** are a special category within the tag system: they have **only a value and no namespace**.

Their purpose is not to describe a card's intrinsic attributes, but to describe **"which event group the result of this wish belongs to"**. Common examples include `up` (rate-up), `standard` (standard), `fes` (festival-limited), and `appoint` (appointed).

Why are event tags needed? Consider a typical scenario:

> The same 5-star character card might be just a regular card (`standard`) in **Banner A**, but become a rate-up card (`up`) in **Banner B**.

If "whether it is UP" were hard-coded onto the card, that dynamism would be lost — a single card could not belong to two different event groups at the same time.

Event tags are designed precisely to solve this: they are **not attached to the card itself**, but are **dynamically produced by the wish logic on each draw**. The same card naturally has a different event grouping in different banners.

### 2.3 Tagging

All core objects in `Wishes` — cards, decks, banners, and logic — can **carry a set of tags**. When you describe an object, tags are the "keywords" you attach to it.

For example, a card might simultaneously carry `rarity:5`, `type:character`, and `game:genshin`. The system understands "what this card is" precisely through these tags.

## 3. Sources and Forking

### 3.1 Three Sources

All core objects in `Wishes` (cards / decks / logic / banners) carry a **source** field, indicating where they come from:

| Source | Meaning | Storage Location | Writable |
| :--- | :--- | :--- | :---: |
| `official` | Official data | `data/official/` | ✗ |
| `local` | User data | `data/local/` | ✓ |
| `pack:<name>` | Pack data | `data/packs/<name>/` | ✗ |

Official data and pack data are **always read-only** at runtime; all user creations are written to `local`.

### 3.2 Forking: How to Edit Read-only Objects

When you want to modify an object from a **read-only source** (for example, renaming an official card), `Wishes` does not modify it in place. Instead, the system will:

1. Copy all contents of that object.
2. Assign a new `GlobalId`.
3. Change the source to `local`, and record `forked_from` pointing to the original object.
4. Write the forked copy into `data/local/`.

The original object remains unchanged and is **never modified**. This process is called **forking**.

The benefits of forking are:

- **Official data is never polluted** — even if you make a mistake, official data remains intact.
- **Conflicts are impossible** — user data and official data are physically separated, so official updates will not overwrite your edits.
- **Traceable** — a forked object records its origin via `forked_from`, making it clear which official card it was derived from.

### 3.3 Shadow: Which Version is Currently Active

After forking, the original and the forked object both exist in the system, but they **should not both be active at the same time** — otherwise a wish might randomly draw either version, leaving the user confused: "I edited it, so why am I still getting the old version?"

**Shadow** is the switch that solves this:

- **The forked object shadows the original (default)**:
  - The original is **hidden** from card lists and banner draws.
  - Deck `include_ids` / `exclude_ids` that reference the original are redirected to the forked object **at resolution time** by the local overlay; deck files are never modified.
  - Only the forked object can be drawn.
- **The forked object does not shadow the original**:
  - Both versions are **visible** in lists.
  - Deck references keep pointing to the original.
  - Drawing yields the original; the forked object behaves more like a "draft".

Users can toggle the shadow state at any time:

- **Remove shadow**: The original becomes visible again, and references resolve back to the original. Drawing immediately reflects the original version.
- **Re-enable shadow**: The original is hidden again, and references resolve to the forked object again.

> [!NOTE]
>
> **Reference overlay**: Reference redirections caused by shadowing are never written back to JSON files. They are stored in the local database `db/overrides.db`, with one table per object kind (Card / Deck / Logic; Card is currently active). Banner-to-deck reference overrides will be enabled together with Banner CRUD.

The meaning of shadow can be summarized in one sentence: **"Which version is currently in effect"**.
It does not change how data is stored; it only determines which version participates in list display and drawing.

> [!NOTE]
>
> In the current UI, **cards** can be shadowed or not when editing, and the shadow state can be toggled at any time from the detail panel. **Decks** default to shadowing on fork; manual toggling will be added in a later version.

### 3.4 Deleting a Forked Object

After deleting a forked object:

- The original **automatically becomes visible again**.
- The reference overlay is removed: deck references to the original take effect again, and deck files were never modified.

In other words, deleting a forked object is equivalent to "abandoning your edits and returning to the official version". If a deck explicitly referenced the forked object's own Id (e.g. a deck created after the fork), that reference becomes dangling, just like deleting any local card.

### 3.5 At Most One Fork per Original

To avoid confusion in reference relations, `Wishes` enforces: **each original object can have at most one forked object**.

If you have already forked an official card, attempting to edit it again will be rejected, and you will be prompted to edit the existing forked version directly. If you want to start over from the official version, you must **delete the forked object first**, then fork again.

## 4. Cards

**Cards** are the most basic output of the wish flow.

Each card has the following core fields:

- **GlobalId** — a globally unique identifier (Uuid v7), generated at creation time and never changed.
- **Source** — as described above: `official` / `local` / `pack:<name>`.
- **Content** — the text displayed for the card, such as a character name or item name. Content is a **localized map** (locale → text), for example carrying both `zh-CN: "胡桃"` and `en: "Hu Tao"`.
- **Title / Subtitle** — optional field, also a localized map.
- **Tags** — used to describe its attributes.

Content may be duplicated — two cards can share the same name as long as their `GlobalId`s differ.

> [!NOTE]
>
> An **artwork** (art asset) field for cards is planned for a future version.

## 5. Decks

A **deck** is the definition of a "candidate card pool". Its defining characteristic is:

> A deck **does not store a list of cards**; instead, it **dynamically computes its members through a set of filter rules**.

This means that when cards change (for example, when a new 5-star character is added), the deck will automatically include the matching cards without manual maintenance.

### 5.1 Membership Rules

Membership rules consist of an **ordered** set of conditions, applied in the order they are declared. Conditions fall into the following categories:

**Inclusion conditions** — search globally for matching cards and merge them into the current result:

- **Has all tags** — find cards that have all of the specified tags.
- **Has any tag** — find cards that have any of the specified tags.
- **Specific GlobalIds** — directly add cards with the specified identifiers.

**Filter conditions** — further narrow down the current result:

- **Keep all tags** — within the current result, keep only cards that have all of the specified tags.
- **Keep any tag** — within the current result, keep only cards that have any of the specified tags.

**Exclusion conditions** — remove from the result:

- **Exclude specific GlobalIds** — directly remove cards with the specified identifiers.

#### Execution Order

The **declaration order of conditions matters**. The system processes them in two phases:

1. **Phase 1**: Apply all **inclusion** and **filter** conditions in the order they are declared.
   Order matters here — "Has all tags" merges into the current result, whereas "Keep all tags" intersects with the current result. Swapping their positions can lead to different results.
2. **Phase 2**: Apply all **exclusion** conditions together.
   No matter where "Exclude specific GlobalIds" is placed, it takes effect at the end.

In the end, the deck's members are the set of cards left after all the above conditions have been applied.

### 5.2 Event Groups

**Event groups** are a **further narrowing** on top of the members, describing "which cards are actually included under a given event tag".

For example, a banner might contain "all 5-star characters", some of which are marked as `up` (rate-up) and the rest as `standard`. Event groups are the rules that define "which members belong to `up` and which belong to `standard`".

The condition types for event groups are essentially the same as for membership rules, with the following additional options:

- **All** — take all members.
- **Reference other event groups** — reuse the rules of other event groups (*not yet implemented*).

#### An Important Constraint

> The result of an event group **must be a subset of the deck's members**.

Even if an event group's conditions use global searches such as "Has all tags", the final result will be intersected with the deck's members. This guarantees that no card appears in an event group that does not belong to the deck.

### 5.3 Card Queries

When the system needs to "find candidate cards based on tags", it performs the following steps in order:

1. Compute all members of the deck.
2. If the wish logic provides regular tag conditions, intersect them with all cards in the catalog that match those tags.
3. For each event tag, look up the corresponding event group and filter further; **if an event tag has no corresponding event group, return an empty result immediately**.
4. Return all card identifiers that satisfy the conditions.

This process is handled automatically by the program; you only need to declare the rules when configuring a deck.

## 6. Wish Logic

**Wish logic** is responsible for producing "what type of card this draw should hit". Its output is expressed as **tags**, not as specific cards.

The logic consists of two parts: a **static definition** and a **runtime state**.

### 6.1 Logic Definition

A **logic definition** is a static blueprint, describing "what this logic is" and "which implementation approach it uses".

`Wishes` supports two kinds of logic implementations:

- **Hard-coded logic** — implemented directly inside the program, with good performance, suited for simulating the complex mechanics of a specific game (such as Genshin's Capturing Radiance).
- **Rule-chain logic** — composed of a series of combinable rules (*rule executors are still under development*).

Currently, `Wishes` includes the following hard-coded logics:

| Name | Description |
| :--- | :--- |
| Genshin Impact Character UP | Includes the Capturing Radiance mechanic |
| Honkai: Star Rail Character UP | Standard large/small pity |

### 6.2 Runtime State

A **runtime state** is the progress of a logic definition **within a specific banner**.

For example, the logic state for Genshin Impact Character UP records:

- How many draws since the last 5-star
- How many draws since the last 4-star
- Whether the next 5-star is guaranteed (large pity)
- The Capturing Radiance counter

Please note, however: **logic state (a JSON object freely defined by the logic)** and **banner runtime state** are two different layers. The banner runtime state is a structured object containing:

- **Total draw count** — how many draws have been made under this scope.
- **Logic state** — a JSON object freely defined by the logic definition and updated during logic execution.

The banner runtime state is keyed by **state scope** (see the next section). A logic definition can be shared across multiple banners, but each scope's state is **independent**.

### 6.3 Logic Result

After the logic finishes executing, it produces a **logic result**, which contains two groups of information:

- **Regular tag conditions** — describes "what type of card was drawn", for example "5-star character".
- **Event tag conditions** — describes "which event group this draw belongs to", for example `up` or `standard`.

The logic result is the **interface between the logic and the deck**: the logic only cares about which tag combination it produces, and the deck only cares about which cards those tags can match.

## 7. Banners

A **banner** is the user-facing entry point for wishing. It binds together a **deck** and a **logic definition**.

One banner = **one deck** + **one logic definition** + **one state scope**.

- Multiple banners can reference **the same deck** — for example, a "Genshin Character UP banner" and a "Genshin Character Standard banner" may share a candidate card pool.
- Multiple banners can also reference **the same logic definition** — for example, several UP banners all use the "Genshin Character UP" logic.
- Each banner's **state scope** determines whether its runtime progress is shared with other banners.

### 7.1 State Scope

A state scope is a string key with two possible forms:

- `banner` (default): each banner maintains its own state **independently**. Modifying one banner's pity counter does not affect other banners.
- `group:<name>`: multiple banners in the same group **share** a single state. For example, the pity progress of Arknights standard banners inherits across different UP banners, so they can be grouped into the same `group`.

This is the foundation for mechanisms such as "pity inheriting across banners".

## 8. What Happens During a Single Wish

When you click the "Wish" button in the interface, `Wishes` performs the following steps in order:

**Step 1: Lock the banner.**
The system obtains the target banner and temporarily locks it, preventing two wish requests from modifying the same state at the same time.

**Step 2: Read the counter.**
The system reads "how many times this scope has been drawn from" from the **state scope** corresponding to the banner.

**Step 3: Generate the random seed.**
The system mixes "user + banner + draw count" into a **deterministic random seed**.
Because the seed is deterministic, the same input always produces the same output — meaning **every draw is reproducible**.

**Step 4: Execute the wish logic.**
The logic definition runs the corresponding implementation, producing a logic result (containing regular tag conditions and event tag conditions), and updates the logic state.

**Step 5: Query candidate cards.**
The logic result is handed to the deck, which filters out all matching cards based on the tag conditions.

**Step 6: Randomly select one.**
Among all candidate cards, one is selected with **equal probability**.

**Step 7: Update state.**
"Draw count +1" along with the new logic state is written back to the **state scope** and persisted to `SQLite`.

**Step 8: Return the result.**
The drawn card (along with the event tags for this draw) is returned to the interface for display.

### About Determinism

At present, wishing in `Wishes` is **reproducible**: as long as the user, banner, and draw count are identical, the result is guaranteed to be identical. This design brings two benefits:

- **Auditable** — any draw result can be re-verified.
- **Testable** — developers can precisely verify boundary behavior (such as "the 90th draw must produce a 5-star").

> [!NOTE]
>
> The current random seed in `Wishes` is a simplified implementation, built as `local user Id + banner Id + cumulative draw count`.
>
> In the future, the way the random seed is constructed will be changed so that results differ across devices, while remaining reproducible.

## 9. How the Concepts Relate

In one sentence, here is how these concepts cooperate:

> A **banner** references a **deck** and a **logic definition**, and is associated with a **state scope**. The **logic** executes and produces a **logic result**. The **logic result** is handed to the **deck**, which uses the tag conditions to filter out **cards**. The card that is ultimately selected is the result of this draw.

More specifically:

- **Cards** are static data, carrying tags, localized content, and a source.
- **Decks** are sets of tag-based rules that dynamically compute their members.
- **Logic definitions** are static blueprints for wish mechanics.
- **State scopes** are the persistence unit of banner progress, either independent or shared across multiple banners.
- **Logic results** are the intermediate product through which the logic and the deck communicate.
- **Banners** are the combination of all the above elements, and are what the user interacts with directly.
- **Forking** is how users edit read-only source objects, and it keeps official data free of contamination.

## 10. Creating a Custom Banner

Suppose you want to simulate a banner from a custom game. The general steps are as follows.

### Step 1: Prepare Cards

Add cards to the cards folder in the data directory, giving each card content and tags.

If you are using the graphical interface, you can create them directly in the *Catalog → Cards* panel.

### Step 2: Create a Deck

Create a new deck and declare:

- **Membership rules** — defines which cards belong to this deck.
- **Event group rules** — defines which cards are included in each event group.
- **Tags** (optional) — tags attached to the deck itself.

### Step 3: Choose or Define Wish Logic

You can use the hard-coded logic built into `Wishes`, or wait for the upcoming custom rule-chain feature.

> [!NOTE]
> The built-in logic currently supports the Genshin Impact and Honkai: Star Rail character UP banners.

### Step 4: Create a Banner

Create a banner, bind the deck and logic together, set an initial state for the logic (for example, an initial pity count of 0), and specify a state scope (`banner` or `group:<name>`).

### Step 5: Launch and Validate

After launching `Wishes`, the loader will automatically perform the following checks:

1. **Data format version** — reads `version.json` at the root of the data directory; if the version does not match, it fails immediately (**no automatic migration**).
2. **GlobalId uniqueness** — the `GlobalId` of every object must be unique across all sources.
3. **Source consistency** — the source declared inside a data file must match the physical directory it lives in.
4. **Reference integrity** — the deck and logic definition referenced by the banner must exist.
5. **Coverage validation** — the deck must match **all possible output combinations** of the logic. If any output combination finds no candidate cards, loading will fail and report the missing combination.

If validation passes, the banner will appear on the home page and you can begin wishing.

> [!NOTE]
> The editing UI for banners and wish logic will be completed in a later version.

## 11. Further Reading

- [Project README](../README.md)
- [Quick Start](../README.md#quick-start)
