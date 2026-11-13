---
"@tellegen/engine": patch
---

The AC power flow solves ZIP and exponential loads at the solved voltage, inside the Newton iteration with their exact voltage derivatives, and its sensitivities include that dependence. A solve with such loads reports them as `load_voltage_dependent`. SOCWR still refuses them, with a message that names the AC power flow.
