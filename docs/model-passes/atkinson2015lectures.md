# atkinson2015lectures: model passes and follow-ups

The model passes of this book are recorded in `docs/model-merge-proposal.md` §17. This file holds later follow-ups.

## Follow-up: open items fixed (2026-11-21)

The owner asked for the open items to be solved. The work was done by hand on the live box through the zt tools, after the box backup `D:/zt/backups/zt-before-small-items-2026-11-21.sqlite3`.

**Income elasticity in Lecture 10 (the old open item "13|f").** The Cards have moved since the item was recorded. The Card is now `13|e|a|a|4` ("瑞士城市估计：收入弹性约为 1…"), and its Finding is `13|e|a|b` ("…收入弹性为 1.29"). The source (Lecture 10, the test of the median voter model, Scholar Archive files 100 and 101) states both values. The Pommerehne–Schneider equation has the coefficient 1.29 on $\log_e Y_m$, and Table 10–1 lists 1.29 as the income demand elasticity. The text then says that the coefficients "imply an income elasticity of around unity". No Card misstated a number, so no number was changed. The wording was made exact in three Cards: `13|e|a|a|4` now says that "约为 1" is the authors' summary of the coefficient 1.29; the Finding `13|e|a|b` says that the authors summarize its 1.29 estimate as an elasticity of about 1; and the model example `13|b|a|a|a|3|a|b` says that the income coefficient 1.29 is the income elasticity, which the authors call about 1.

**R13 on `19|b|a|a|a|d`.** The setting card "设定：固定收入的家庭、社会福利函数与公共品的预算约束" (model `19|b|a|a`) had 28 own sentences, and the limit is 24. R13 excludes only **直觉**, not **说明**. The card was therefore split by `zt-model-settings`. Four setting items moved to a new Side setting card, `19|b|a|a|a|d|b` "设定：政府信息完全、一次决策，家庭收入外生" (facet `设定：信息与时序`, aside). Those items are the static timing, the government's full information, the exogenous income, and the price normalization with its （补：书中未写） mark. The new card has a **起点** that rests on `|d`, a **设定** section and a three-line **直觉**. The Model Card's **结构** lists the new part. No formula was changed.

**Gates.** `19|b|a|a` and `13|b|a|a`: check-model self_sufficient true, check-network clean, check-premises none, check-settings clean, `imports.mjs check` exit 0. zt-check reports no R13 fault. The language advisories it still reports (R2, R3, R20, R21 on other parts of `19|b|a|a`; F2 and L1 on the Finding `13|e|a|b`) were not written by this pass. `list_broken_links` is empty, and the Term relink added 4 Links in 1 Card.
