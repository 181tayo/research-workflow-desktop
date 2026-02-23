1) Variables
DV: `dv_choice`
IV: treatment_group
Controls: age, baseline_score

2) Main analysis
regress dv_choice on treatment_group + age + baseline_score

3) Exclusions
exclude participants with duration < 60
