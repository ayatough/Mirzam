---
title: Compensating thermal drift in a fibre Bragg grating strain array
author: R. Wren(Instrumentation Group)
aspect: "16:9"
theme: mirzam
mode: dark
bibliography: refs-research.bib
citation-style: numeric
vars:
  sensors: 16
  drift_before: 38
  drift_after: 4
  span_m: 120
---

# Compensating thermal drift in a<br>fibre Bragg grating strain array {.title-slide}

Group meeting — Instrumentation

R. Wren|2026-08-13

<!-- note: One sentence of framing: the array works, the numbers move with the weather, and this is what we did about it. -->

---

```pane
+------------------------------------+
|  head                              |
+------------------------------------+
|                                    |
|                                    |
|  main                              |
|                                    |
|                                    |
+------------------------------------+
```

::: pane head
## What this covers
:::

::: pane main {valign=middle}
```toc
from: 2
current: true
```

*Twelve minutes, then questions — each line links to its slide*
:::

---

```pane
+------------------+-----------------+
|  head                              |
+------------------+-----------------+
|                  |                 |
|  main            |  fig            |
|                  |                 |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Background]{.eyebrow}
## One grating, two things to measure
:::

::: pane main
- A grating's reflected wavelength moves with **strain** and with
  **temperature**, and one reading cannot tell you which moved it[@alder2019]
- Over our {{span_m}} m baseline the diurnal swing is worth more than the
  signal we are trying to see
- So the array is only as good as the temperature model behind it — which is
  the part nobody publishes

> A strain number without a temperature number is an opinion.
:::

```shape
rect #fibre at(74%, 55%) size(34%, 6%) label="fibre" radius=6
rect #g1    at(64%, 55%) size(6%, 6%) stroke=@accent2
rect #g2    at(74%, 55%) size(6%, 6%) stroke=@accent2
rect #g3    at(84%, 55%) size(6%, 6%) stroke=@accent2
text at(74%, 70%) "{{sensors}} gratings, one fibre" .small
```

<!-- note: Do not derive the wavelength shift here; slide 5 has it. -->

---

```pane
+------------------------------------+
|  head                              |
+------------------------------------+
|                                    |
|  table                             |
|                                    |
+------------------------------------+
|  note                              |
+------------------------------------+
```

::: pane head
[Background]{.eyebrow}
## Three ways people compensate, and what each costs
:::

::: pane table {valign=middle}
| Approach | Extra hardware | Holds over | Our verdict |
|---|---|---|---|
| Reference grating, unstrained | One channel per zone | Slow drift | Channel budget too tight[@devi2022] |
| Athermal packaging | Custom mounts | Wide range | Cannot retrofit the installed array[@castellani2021] |
| Dual-wavelength matrix | None | Calibrated range | **Chosen** — arithmetic, not parts[@alder2019] |
:::

::: pane note {align=right}
*See [@devi2022] for a survey of the field; this table is only the part that bears on a retrofit.*
:::

---

```pane
+------------------------------------+
|  head                              |
+------------------------------------+
|                  |                 |
|  main            |  eq             |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Method]{.eyebrow}
## Two wavelengths, one matrix
:::

::: pane main
- Each sensing point carries **two gratings** with different photo-elastic and
  thermo-optic coefficients
- Their shifts are a linear system in $(\varepsilon, \Delta T)$, so inverting a
  2×2 matrix separates them[@alder2019]
- The matrix is fitted once per installation, in an oven, over the range the
  site actually sees
:::

::: pane eq {valign=middle}
$$
\begin{pmatrix} \Delta\lambda_1 \\ \Delta\lambda_2 \end{pmatrix}
=
\begin{pmatrix} K_{\varepsilon 1} & K_{T1} \\ K_{\varepsilon 2} & K_{T2} \end{pmatrix}
\begin{pmatrix} \varepsilon \\ \Delta T \end{pmatrix}
$$

$$
\varepsilon = \frac{K_{T2}\,\Delta\lambda_1 - K_{T1}\,\Delta\lambda_2}{K_{\varepsilon 1}K_{T2} - K_{\varepsilon 2}K_{T1}}
$$
:::

<!-- note: The denominator is the conditioning question someone always asks. Answer: 0.02 in our coefficient set, and that is slide 7. -->

---

```pane
+------------------------------------+
|  head                              |
+------------------------------------+
|                  |                 |
|  chart           |  note           |
|                  |                 |
+------------------+-----------------+
```

::: pane head
[Results]{.eyebrow}
## Apparent strain over a day, before and after
:::

::: pane chart
```chart
type: line
id: drift
title: Apparent strain with no load (µε)
y_label: µε
data: |
  hour, uncompensated, compensated
  00, 6, 1
  04, 12, 2
  08, 21, 2
  12, 38, 4
  16, 31, 3
  20, 11, 1
```
:::

::: pane note {valign=middle}
- Peak apparent strain falls from **{{drift_before}} µε** to
  **{{drift_after}} µε**
- What is left tracks the *rate* of temperature change, not its value —
  a lag, not a scale error
- Across all {{sensors}} gratings the worst channel is 6 µε
:::

<!-- note: The residual shape is the interesting part. Do not claim it as noise. -->

---

```pane
+------------------------------------+
|  head                              |
+------------------------------------+
|                                    |
|  main                              |
|                                    |
+------------------------------------+
```

::: pane head
[Discussion]{.eyebrow}
## Where it still fails
:::

::: pane main
- **Outside the calibrated range** the matrix is an extrapolation, and the
  coefficients are not linear there — the same limit the survey
  reports[@devi2022]
- **Fast transients** beat the lag: a cold front moving over the span leaves
  the two gratings at different temperatures, and the model assumes they are
  not
- Published drift budgets put a retrofit like ours near the floor of what is
  useful[@erikson2023], which is an argument for packaging on the next
  array[@castellani2021] rather than for a better fit on this one
:::

<!-- note: If asked about the transient case: we have four events on record and none of them are in this dataset. -->

---

```pane
+------------------------------------+
|  head                              |
+------------------------------------+
|                                    |
|  refs                              |
|                                    |
|                                    |
+------------------------------------+
```

::: pane head
## References
:::

::: pane refs
```bibliography
```
:::

<!-- note: Numbered in citation order; the ↩ on each entry is the slide it was cited on, so a question about [1] can go straight back to the method. -->

---

```pane
+------------------------------------+
|                                    |
|  main                              |
|                                    |
+------------------------------------+
|  foot                              |
+------------------------------------+
```

::: pane main {align=center valign=middle}
## Summary

Two gratings and a 2×2 inverse, no new hardware

Apparent drift **{{drift_before}} µε → {{drift_after}} µε** across {{sensors}} channels

*Next: the transient case, and whether packaging is worth it on array two*
:::

::: pane foot {align=right}
Instrumentation group|2026-08-13
:::

<!-- note: Ask for the oven slot before finishing; that is the blocker. -->
