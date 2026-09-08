# Segment tiling, worked through

How charging sessions land on the 15-minute segments that an interval of interest is divided
into, walked through on the seven-session example in `tests/fixtures/Session_Report_Diagram.csv`.

The same example is asserted, session by session, in `tests/session/segment_tiling.rs`, and rendered in
full in `tests/fixtures/sessions/Session_Report_Diagram.report.md`. This document is the prose; those two
are the machine-checked versions of the same claims.

There is no diagram beyond the sketch below, and deliberately so. A uniform 15-minute partition is
simple enough to read as a table — what needed a drawing was the old variable-length grouping,
where the group boundaries were themselves derived from the data.

## The interval and its segments

The interval of interest is 16:00–17:00 local on 2026-06-15, a date with no DST transition. It is
one hour, so it divides into four segments:

| Segment | From  | To    |
|---------|-------|-------|
| `16:00` | 16:00 | 16:15 |
| `16:15` | 16:15 | 16:30 |
| `16:30` | 16:30 | 16:45 |
| `16:45` | 16:45 | 17:00 |

Segments are **half-open**: each runs from its own start up to but *not including* the next one's.
No instant falls in two of them, and together they cover the interval exactly. A 15-minute interval
of interest yields a single segment by the same rule.

Segments are named by the minute they start on, and that name is the join key across the report:
the Estimates section names the winning segment by it, the Segments table lists it, and the Segment
membership section keys its lists on it.

## The seven sessions

Every session occupies the half-open span `[Conn_DateTime_Start, Conn_DateTime_End)`, exactly as
the report states them. The portal states both to the second, so there is nothing to adjust.

The clock times throughout this document are the ones the *report* states, which are on standard
time all year. A rendered report shows the same instants in prevailing local time, so its segments
are named an hour later — the segment called 16:15 here is `17:15` in the tables below.

```text
           16:00      16:15      16:30      16:45      17:00
             |          |          |          |          |
  A   15:54 =|==========|==========|==========|==========|===== 17:04
  B     15:59|=====|16:16                                        overruns the left edge
  C          |  16:08 =====|16:43                                nested, spans two segments
  E          |         16:20 ==|16:35                            staggered start with D
  D          |          16:24 =|16:35                            ends the same minute as E
  F          |            16:34 =====|16:43                      starts the minute D and E end
  G          |                       16:48 ==|16:56              alone in the last segment
```

| Session | Reported start | Reported end | Span used            | Notes                          |
|---------|----------------|--------------|----------------------|--------------------------------|
| `A`     | 15:54          | 17:03        | 15:54 – 17:04        | Outruns the interval both ends |
| `B`     | 15:59          | 16:15        | 15:59 – 16:16        | Starts before the interval     |
| `C`     | 16:08          | 16:42        | 16:08 – 16:43        | Wholly inside                  |
| `E`     | 16:20          | 16:34        | 16:20 – 16:35        | Staggered against `D`          |
| `D`     | 16:24          | 16:34        | 16:24 – 16:35        | Ends the same minute as `E`    |
| `F`     | 16:34          | 16:42        | 16:34 – 16:43        | Starts the minute `D`/`E` end  |
| `G`     | 16:48          | 16:55        | 16:48 – 16:56        | Alone but for `A`              |

## Which sessions each segment holds

A session belongs to a segment when the two **overlap** — share at least one instant. Abutting is
not overlapping, and the half-open convention is what keeps the two distinguishable.

| Segment | Sessions              | Why                                                        |
|---------|-----------------------|------------------------------------------------------------|
| `16:00` | `A`, `B`, `C`         | `D`, `E`, `F`, `G` all begin after 16:15                    |
| `16:15` | `A`, `B`, `C`, `D`, `E` | `B` reaches in by one minute; `D` and `E` start inside    |
| `16:30` | `A`, `C`, `D`, `E`, `F` | `B` has ended; `F` starts inside                          |
| `16:45` | `A`, `G`              | `C`, `D`, `E`, `F` have all ended by 16:43                  |

Two entries are worth dwelling on.

**`B` is not in the 16:15 segment.** `B` ends at 16:15, exactly where the segment starts. Spans are
half-open, so that instant belongs to the segment and not to `B`: the two abut and do not overlap.
`B` used to be counted here, because its reported end was padded a minute forward on the reasoning
that a time stated to the minute could mean anywhere inside it.

**`D`, `E` and `F` in the 16:30 segment.** `D` and `E` end at 16:34 and `F` starts at 16:34, so `F`
abuts them rather than overlapping them — the shared instant is `F`'s. All three still meet the
segment, each covering a different part of it.

## What each segment contributes

Two aggregates are computed per segment, and every estimate is derived from them:

- **`agg_count`** — each session's *overlap ratio*, the fraction of the segment its span covers,
  summed over the segment's sessions. A session covering the whole segment contributes 1; one
  covering half contributes 0.5. It is a session count weighted by presence, so it is fractional.
- **`agg_kw`** — each session's `Energy_Use` spread evenly over its own connection span, the part
  of it falling in this segment summed the same way, and the total divided by the segment's length
  in hours.

The two divide the same overlap by different things — the segment's own length for the count, the
session's for the energy — so a short heavy session and a long light one can rank differently in
them. `Active_Charge_Time` takes no part in either.

Both are single numbers. Reported times are exact, so an overlap has one width. They were a pair of
bounds while those times were stated only to the minute and a session's edge could lie anywhere
inside the minute it named.

| Segment | `agg_count` | `agg_kw` |
|---------|------------:|---------:|
| `16:00` |       2.467 |   14.553 |
| `16:15` |       3.067 |   17.482 |
| `16:30` |       2.867 |   16.165 |
| `16:45` |       1.467 |    8.440 |

## Which segment is reported

The estimates are reported for the **maximal** segment: the one where the derivation peaks. The
two derivations are ranked separately and need not agree, so each names its own segment.

Here they do agree, and narrowly. 16:15 leads 16:30 by 0.200 on `agg_count` (3.067 against 2.867)
and by 1.317 kW on `agg_kw` (17.482 against 16.165). Both maxima are 16:15, and that is the segment
the report names.

The narrowness is the point rather than an accident of the fixture. Two segments this close mean
that a small change in the fixture — one session a minute longer — would move the winner, so a
reader quoting one figure should look at the Segments table rather than treat the winner as
decisive.

Ties go to the earliest segment. That is not a rare case to have decided: in an interval no session
reached at all, every segment sits at the same standing block, and without a rule the report would
name an arbitrary one.

## The empty case

An interval that no session intersects still has its four segments, and they are not zero. The
transformer is energised whether or not a vehicle is plugged in, and its core loss and magnetizing
current are part of the building's demand. Every segment reports one standing block per installed
panel — `single_panel_load(0.0)` taken `PANEL_COUNT` times — and the report says in prose why the
figures are not zero.
