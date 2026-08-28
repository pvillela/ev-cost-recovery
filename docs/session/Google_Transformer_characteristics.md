# Google: Transformer characteristics

## Question 1

**Can Core loss, constant whenever energised, Copper loss at rated load, Magnetizing current, per unit of rating, Leakage reactance, per unit of rating be determined from a transformer's kVA nameplate?**

No, you cannot determine core loss, copper loss, or magnetizing current from just a transformer's kVA nameplate rating.

### What a Nameplate Tells You

A standard transformer nameplate shows basic operating limits, not internal electrical losses or test parameters.

- **Apparent power rating (kVA):** The maximum power the transformer can handle.
- **Voltages:** The primary and secondary voltage levels.
- **Percent impedance (%Z):** Listed on many nameplates, showing the voltage drop under load.

### What Cannot Be Determined From the Nameplate

These values require specific laboratory tests, such as open-circuit and short-circuit tests, rather than a simple rating label.

- **Core loss:** Energy lost in the iron core whenever the transformer is energized; found using an open-circuit test.
- **Copper loss:** Energy lost as heat in the copper windings at rated load; found using a short-circuit test.
- **Magnetizing current:** The small current needed to create the magnetic field in the core; found using an open-circuit test.

### What Can Be Partially Found

- **Leakage reactance (per unit):** You can estimate this from the nameplate's **percent impedance (%Z)** if the winding resistance is very small and can be ignored.

If you are working on a specific **transformer problem** or looking at a **test report**, let me know what data you have and I can help you calculate these values.



## Question 2

**The transformer is a 75 kVA dry-type unit.**

Knowing that this is a **75 kVA dry-type transformer** allows you to estimate typical industry-standard values, though the exact figures still depend heavily on the manufacturer, efficiency ratings (such as DOE 2016 standards), and the temperature rise rating. [[1](https://forums.mikeholt.com/threads/75-kva-transformer-load-with-no-load.56326/), [2](https://www.google.com/search?q=product&prds=pvt:hg,productid:15062986638910315517,catalogid:6750760167949745066,gpcid:6072729995640986560&ibp=oshop)]

Typical performance parameters for a standard 75 kVA, 3-phase, 150°C temperature-rise dry-type transformer include:

### 1. Core Loss (Constant/No-Load Loss)

- **Typical value:** **300 to 450 Watts** (approx. 0.4% to 0.6% of rating).
- **Context:** This loss occurs entirely in the silicon steel core and remains constant 24/7 as long as the transformer is energized, regardless of how much electrical load is connected. Modern, highly energy-efficient models hover closer to the 300W mark. [[1](https://www.csemag.com/transformer-efficiency-minimizing-transformer-losses/), [2](https://www.rexpowermagnetics.com/knowledge-hub/understanding-losses-in-transformers/), [3](https://electrical-engineering-portal.com/download-center/electrical-software/transformer-losses-calculation), [4](https://taishantransformer.com/power-transformer-no-load-and-load-losses/), [5](https://forums.mikeholt.com/threads/75-kva-transformer-load-with-no-load.56326/)]

### 2. Copper Loss (Load Loss at Rated Load)

- **Typical value:** **1,800 to 2,500 Watts** (approx. 2.4% to 3.3% of rating).
- **Context:** This is the \(I^{2}R\) resistive heating loss occurring inside the primary and secondary aluminum or copper windings when the unit operates at 100% capacity. If the transformer is loaded at 50% capacity, the copper loss drops quadratically to 25% of this value. [[1](https://forums.mikeholt.com/threads/75-kva-transformer-load-with-no-load.56326/), [2](https://www.eng-tips.com/threads/transformer-heat-loss.162961/), [3](https://www.larsonelectronics.com/articles/detail/860), [4](https://www.facebook.com/groups/195259792384293/posts/1296616335581961/), [5](https://calcpanel.com/guides/transformer-efficiency-and-loss-calculation)]

### 3. Magnetizing Current (Per Unit)

- **Typical value:** **0.01 to 0.03 per unit** (1% to 3% of the rated full-load current).
- **Context:** This is the small steady-state current drawn from the utility line just to magnetize the iron core. It lags the voltage heavily with a low power factor (typically around 0.1 to 0.2 PF). *(Note: Do not confuse this with brief initial inrush current, which can peak at 8 to 12 times the rated current).* [[1](https://forums.mikeholt.com/threads/dry-transformer-no-load-consumption.145421/), [2](https://taishantransformer.com/transformer-efficiency-measurement-optimization/), [4](https://www.larsonelectronics.com/articles/detail/2314?srsltid=AfmBOor3nDlo-hPliL0z8P-w4QU_cFx7H4hPCSmEPV_PHvOoUzOnf25g)]

### 4. Leakage Reactance (Per Unit)

- **Typical value:** **0.035 to 0.055 per unit** (3.5% to 5.5% of rating).
- **Context:** A standard 75 kVA dry-type transformer typically features a **Percent Impedance (%Z)** stamped on its nameplate of around 4.0% to 6.0%. Since winding resistance is very small, the per-unit leakage reactance (\(X_{pu}\)) is mathematically nearly identical to the total per-unit impedance (\(Z_{pu}\)).

------

To narrow down these estimates into exact figures, let me know if you have access to any of the following details:

- The **manufacturer name** or specific **model number**
- The **Percent Impedance (%Z)** stamped on the nameplate
- Any data from a factory **certified test report**



## Question 3

**Marcus AMTH75A1, IMP% 4.2**

Based on the official technical datasheet for the **Marcus AMTH75A1** (a 3-phase, 75 kVA, 600V Delta to 208Y/120V dry-type transformer meeting DOE 2016 / NRCAN 2019 premium efficiency standards), your metrics can be determined exactly or closely estimated: [[1](https://central.electrozad.ca/products/20-power-distribution/13-transformers/08-power-distribution-transformers/p-TUFSQU1USDc1QTE=-marcus-transformer-amth75a1-distribution-transformer-dry-75-kva-aluminum-winding-3-ph-600-v-nema-3r), [2](https://cdn-e.soneparcanada.io/PIM_Docs/Docs/STEP_ASSETS_PDF/131458876.pdf)]

### 1. Core Loss (No-Load Loss)

- **Exact Value:** **197 Watts**
- **Context:** Because this unit is engineered for modern premium energy standards, its no-load core loss is exceptionally low. It draws a constant 197W 24/7 just by being plugged into the 600V grid, regardless of your actual facility load. [[1](https://central.electrozad.ca/products/20-power-distribution/13-transformers/08-power-distribution-transformers/p-TUFSQU1USDc1QTE=-marcus-transformer-amth75a1-distribution-transformer-dry-75-kva-aluminum-winding-3-ph-600-v-nema-3r), [2](https://cdn-e.soneparcanada.io/PIM_Docs/Docs/STEP_ASSETS_PDF/131458876.pdf), [3](https://cdn-e.soneparcanada.io/PIM_Docs/Docs/STEP_ASSETS_PDF/131458886.pdf), [4](https://electrical-engineering-portal.com/download-center/electrical-software/transformer-losses-calculation)]

### 2. Copper Loss (Load Loss at Rated Load)

- **Exact Value:** **1,293 Watts**
- **Calculation:** The datasheet specifies a **Total Loss of 1,490 Watts** at full load and a **No-Load Loss of 197 Watts**. By subtracting the core loss from the total loss (\(1490\text{W} - 197\text{W}\)), we isolate the \(I^{2}R\) winding losses occurring at 100% rated capacity. [[1](https://www.larsonelectronics.com/articles/detail/860?srsltid=AfmBOopll4NrrlEB4yegaom5qn-TrhDqZ9dimm3ceqXigvvX1T0XDbSu), [2](https://cdn-e.soneparcanada.io/PIM_Docs/Docs/STEP_ASSETS_PDF/131458876.pdf), [3](https://www.facebook.com/groups/195259792384293/posts/1296616335581961/)]

### 3. Leakage Reactance

- **Estimated Value:** **~0.040 to 0.041 per unit** (or **4.0% to 4.1%**)
- **Calculation:** You provided an **Impedance (%Z) of 4.2%** (0.042 per unit). Winding resistance (\(R_{pu}\)) accounts for about 1.72% of the impedance (\(1490\text{W} / 75000\text{VA} \approx 0.0172\)). Using the impedance triangle formula:
  \(\text{Reactance\ }(X_{pu})=\sqrt{Z_{pu}^{2}-R_{pu}^{2}}=\sqrt{0.042^{2}-0.0172^{2}}\approx \mathbf{0.0383}\text{\ per\ unit}\)
  *(Note: This is very close to the standard nameplate's total 4.2% impedance, as leakage reactance dominates the transformer's internal impedance).*

### 4. Magnetizing Current

- **Estimated Value:** **0.01 to 0.02 per unit** (or **1% to 2%** of full load current)
- **Context:** While the exact excitation current is an internal factory test metric not fully spelled out on the public spec sheet, standard premium-efficiency dry-type units of this tier draw an incredibly small magnetizing current to energize the high-grade silicon steel core. [[1](https://www.facebook.com/groups/195259792384293/posts/1296616335581961/), [2](https://www.youtube.com/watch?v=XWLDp63S5vg)]