//! The linked DYR records of a case as named parameters, and edits of single
//! parameter values in the DYR text.
//!
//! A parameter editor needs no engine change: every model reads its values
//! from the DYR text when a session starts. The edit replaces exactly the
//! token of the edited parameter in the record's original text, so every
//! other byte of the file (comments, layout, records tellegen does not read)
//! stays as it was; the edited text is read back to check the edit.
//!
//! @block DYR parameter editing

use serde::{Deserialize, Serialize};

use powerio::{BalancedNetwork, DynamicCase, DynamicRecord};

/// One parameter of a PSS/E model: name, bucket, unit, description.
struct Param(&'static str, &'static str, &'static str, &'static str);

/// PSS/E parameters in record order (PSS/E Model Library) for the models
/// powerio reads (`powerio::SUPPORTED_MODELS`), clustered into buckets for an
/// editor.
const SCHEMA: &[(&str, &[Param])] = &[
    (
        "GENCLS",
        &[
            Param(
                "H",
                "Mechanics",
                "s",
                "inertia constant (MW·s/MVA on the machine base)",
            ),
            Param("D", "Mechanics", "pu", "speed damping"),
        ],
    ),
    (
        "GENROU",
        &[
            Param(
                "T'do",
                "Time constants",
                "s",
                "d-axis transient open-circuit time constant",
            ),
            Param(
                "T''do",
                "Time constants",
                "s",
                "d-axis subtransient open-circuit time constant",
            ),
            Param(
                "T'qo",
                "Time constants",
                "s",
                "q-axis transient open-circuit time constant",
            ),
            Param(
                "T''qo",
                "Time constants",
                "s",
                "q-axis subtransient open-circuit time constant",
            ),
            Param(
                "H",
                "Mechanics",
                "s",
                "inertia constant (MW·s/MVA on the machine base)",
            ),
            Param("D", "Mechanics", "pu", "speed damping"),
            Param("Xd", "Reactances", "pu", "d-axis synchronous reactance"),
            Param("Xq", "Reactances", "pu", "q-axis synchronous reactance"),
            Param("X'd", "Reactances", "pu", "d-axis transient reactance"),
            Param("X'q", "Reactances", "pu", "q-axis transient reactance"),
            Param(
                "X''d",
                "Reactances",
                "pu",
                "subtransient reactance (X''q = X''d)",
            ),
            Param("Xl", "Reactances", "pu", "stator leakage reactance"),
            Param(
                "S(1.0)",
                "Saturation",
                "",
                "saturation factor at 1.0 pu flux",
            ),
            Param(
                "S(1.2)",
                "Saturation",
                "",
                "saturation factor at 1.2 pu flux",
            ),
        ],
    ),
    (
        "GENROE",
        &[
            Param(
                "T'do",
                "Time constants",
                "s",
                "d-axis transient open-circuit time constant",
            ),
            Param(
                "T''do",
                "Time constants",
                "s",
                "d-axis subtransient open-circuit time constant",
            ),
            Param(
                "T'qo",
                "Time constants",
                "s",
                "q-axis transient open-circuit time constant",
            ),
            Param(
                "T''qo",
                "Time constants",
                "s",
                "q-axis subtransient open-circuit time constant",
            ),
            Param(
                "H",
                "Mechanics",
                "s",
                "inertia constant (MW·s/MVA on the machine base)",
            ),
            Param("D", "Mechanics", "pu", "speed damping"),
            Param("Xd", "Reactances", "pu", "d-axis synchronous reactance"),
            Param("Xq", "Reactances", "pu", "q-axis synchronous reactance"),
            Param("X'd", "Reactances", "pu", "d-axis transient reactance"),
            Param("X'q", "Reactances", "pu", "q-axis transient reactance"),
            Param(
                "X''d",
                "Reactances",
                "pu",
                "subtransient reactance (X''q = X''d)",
            ),
            Param("Xl", "Reactances", "pu", "stator leakage reactance"),
            Param(
                "S(1.0)",
                "Saturation",
                "",
                "saturation factor at 1.0 pu flux",
            ),
            Param(
                "S(1.2)",
                "Saturation",
                "",
                "saturation factor at 1.2 pu flux",
            ),
        ],
    ),
    (
        "GENSAL",
        &[
            Param(
                "T'do",
                "Time constants",
                "s",
                "d-axis transient open-circuit time constant",
            ),
            Param(
                "T''do",
                "Time constants",
                "s",
                "d-axis subtransient open-circuit time constant",
            ),
            Param(
                "T''qo",
                "Time constants",
                "s",
                "q-axis subtransient open-circuit time constant",
            ),
            Param(
                "H",
                "Mechanics",
                "s",
                "inertia constant (MW·s/MVA on the machine base)",
            ),
            Param("D", "Mechanics", "pu", "speed damping"),
            Param("Xd", "Reactances", "pu", "d-axis synchronous reactance"),
            Param("Xq", "Reactances", "pu", "q-axis synchronous reactance"),
            Param("X'd", "Reactances", "pu", "d-axis transient reactance"),
            Param(
                "X''d",
                "Reactances",
                "pu",
                "subtransient reactance (X''q = X''d)",
            ),
            Param("Xl", "Reactances", "pu", "stator leakage reactance"),
            Param(
                "S(1.0)",
                "Saturation",
                "",
                "saturation factor at 1.0 pu flux",
            ),
            Param(
                "S(1.2)",
                "Saturation",
                "",
                "saturation factor at 1.2 pu flux",
            ),
        ],
    ),
    (
        "GENSAE",
        &[
            Param(
                "T'do",
                "Time constants",
                "s",
                "d-axis transient open-circuit time constant",
            ),
            Param(
                "T''do",
                "Time constants",
                "s",
                "d-axis subtransient open-circuit time constant",
            ),
            Param(
                "T''qo",
                "Time constants",
                "s",
                "q-axis subtransient open-circuit time constant",
            ),
            Param(
                "H",
                "Mechanics",
                "s",
                "inertia constant (MW·s/MVA on the machine base)",
            ),
            Param("D", "Mechanics", "pu", "speed damping"),
            Param("Xd", "Reactances", "pu", "d-axis synchronous reactance"),
            Param("Xq", "Reactances", "pu", "q-axis synchronous reactance"),
            Param("X'd", "Reactances", "pu", "d-axis transient reactance"),
            Param(
                "X''d",
                "Reactances",
                "pu",
                "subtransient reactance (X''q = X''d)",
            ),
            Param("Xl", "Reactances", "pu", "stator leakage reactance"),
            Param(
                "S(1.0)",
                "Saturation",
                "",
                "saturation factor at 1.0 pu flux",
            ),
            Param(
                "S(1.2)",
                "Saturation",
                "",
                "saturation factor at 1.2 pu flux",
            ),
        ],
    ),
    (
        "SEXS",
        &[
            Param("TA/TB", "Regulator", "", "lead-lag ratio"),
            Param("TB", "Regulator", "s", "lead-lag lag time constant"),
            Param("K", "Regulator", "pu", "regulator gain"),
            Param("TE", "Exciter", "s", "exciter time constant"),
            Param("EMIN", "Limits", "pu", "minimum field voltage"),
            Param("EMAX", "Limits", "pu", "maximum field voltage"),
        ],
    ),
    (
        "ESAC1A",
        &[
            Param("TR", "Measurement", "s", "voltage transducer time constant"),
            Param("TB", "Regulator", "s", "lead-lag lag time constant"),
            Param("TC", "Regulator", "s", "lead-lag lead time constant"),
            Param("KA", "Regulator", "pu", "regulator gain"),
            Param("TA", "Regulator", "s", "regulator time constant"),
            Param("VAMAX", "Limits", "pu", "regulator output maximum"),
            Param("VAMIN", "Limits", "pu", "regulator output minimum"),
            Param("TE", "Exciter", "s", "exciter time constant"),
            Param("KF", "Feedback", "pu", "rate feedback gain"),
            Param("TF", "Feedback", "s", "rate feedback time constant"),
            Param("KC", "Exciter", "pu", "rectifier loading factor"),
            Param("KD", "Exciter", "pu", "demagnetizing factor"),
            Param("KE", "Exciter", "pu", "exciter constant"),
            Param("E1", "Saturation", "pu", "first saturation point"),
            Param("SE(E1)", "Saturation", "", "saturation at E1"),
            Param("E2", "Saturation", "pu", "second saturation point"),
            Param("SE(E2)", "Saturation", "", "saturation at E2"),
            Param("VRMAX", "Limits", "pu", "exciter input maximum"),
            Param("VRMIN", "Limits", "pu", "exciter input minimum"),
        ],
    ),
    (
        "EXST1",
        &[
            Param("TR", "Measurement", "s", "voltage transducer time constant"),
            Param("VIMAX", "Limits", "pu", "error signal maximum"),
            Param("VIMIN", "Limits", "pu", "error signal minimum"),
            Param("TC", "Regulator", "s", "lead-lag lead time constant"),
            Param("TB", "Regulator", "s", "lead-lag lag time constant"),
            Param("KA", "Regulator", "pu", "regulator gain"),
            Param("TA", "Regulator", "s", "regulator time constant"),
            Param("VRMAX", "Limits", "pu", "output maximum (times Vt)"),
            Param("VRMIN", "Limits", "pu", "output minimum (times Vt)"),
            Param("KC", "Limits", "pu", "rectifier loading factor"),
            Param("KF", "Feedback", "pu", "rate feedback gain"),
            Param("TF", "Feedback", "s", "rate feedback time constant"),
        ],
    ),
    (
        "TGOV1",
        &[
            Param("R", "Droop", "pu", "permanent droop"),
            Param("T1", "Valve", "s", "valve time constant"),
            Param("VMAX", "Valve", "pu", "valve maximum"),
            Param("VMIN", "Valve", "pu", "valve minimum"),
            Param("T2", "Turbine", "s", "turbine lead time constant"),
            Param("T3", "Turbine", "s", "turbine lag time constant"),
            Param("Dt", "Damping", "pu", "turbine damping"),
        ],
    ),
    (
        "GAST",
        &[
            Param("R", "Droop", "pu", "permanent droop"),
            Param("T1", "Valve", "s", "fuel valve time constant"),
            Param("T2", "Turbine", "s", "fuel system time constant"),
            Param("T3", "Load limit", "s", "load limit time constant"),
            Param("AT", "Load limit", "pu", "ambient temperature load limit"),
            Param("KT", "Load limit", "pu", "temperature limiter gain"),
            Param("VMAX", "Valve", "pu", "fuel valve maximum"),
            Param("VMIN", "Valve", "pu", "fuel valve minimum"),
            Param("Dturb", "Damping", "pu", "turbine damping"),
        ],
    ),
    (
        "HYGOV",
        &[
            Param("R", "Droop", "pu", "permanent droop"),
            Param("r", "Droop", "pu", "temporary droop"),
            Param("Tr", "Droop", "s", "governor time constant"),
            Param("Tf", "Gate", "s", "filter time constant"),
            Param("Tg", "Gate", "s", "servo time constant"),
            Param("VELM", "Gate", "pu/s", "gate velocity limit"),
            Param("GMAX", "Gate", "pu", "gate maximum"),
            Param("GMIN", "Gate", "pu", "gate minimum"),
            Param("TW", "Turbine", "s", "water time constant"),
            Param("At", "Turbine", "pu", "turbine gain"),
            Param("Dturb", "Damping", "pu", "turbine damping"),
            Param("qNL", "Turbine", "pu", "no-load flow"),
        ],
    ),
    (
        "IEEEST",
        &[
            Param(
                "IC",
                "Input",
                "",
                "input code (1 speed, 3 electrical power on MBASE)",
            ),
            Param("REMBUS", "Input", "", "remote bus (0: own terminal)"),
            Param("A1", "Filter", "s", "torsional filter coefficient"),
            Param("A2", "Filter", "s²", "torsional filter coefficient"),
            Param("A3", "Filter", "s", "torsional filter coefficient"),
            Param("A4", "Filter", "s²", "torsional filter coefficient"),
            Param("A5", "Filter", "s", "torsional filter coefficient"),
            Param("A6", "Filter", "s²", "torsional filter coefficient"),
            Param("T1", "Lead-lag", "s", "first lead time constant"),
            Param("T2", "Lead-lag", "s", "first lag time constant"),
            Param("T3", "Lead-lag", "s", "second lead time constant"),
            Param("T4", "Lead-lag", "s", "second lag time constant"),
            Param(
                "T5",
                "Gain and washout",
                "s",
                "washout numerator time constant",
            ),
            Param("T6", "Gain and washout", "s", "washout time constant"),
            Param("KS", "Gain and washout", "pu", "stabilizer gain"),
            Param("LSMAX", "Output limits", "pu", "output maximum"),
            Param("LSMIN", "Output limits", "pu", "output minimum"),
            Param(
                "VCU",
                "Output limits",
                "pu",
                "terminal voltage above which the output is cut",
            ),
            Param(
                "VCL",
                "Output limits",
                "pu",
                "terminal voltage below which the output is cut",
            ),
        ],
    ),
    // The inverter-based plant (WECC generic renewable models), in the order
    // the engine reads them (`renewable.rs`: PowerSystems.jl's DYR mapping).
    (
        "REGCA1",
        &[
            Param(
                "LVPLSW",
                "Flags",
                "",
                "low-voltage power logic on (1) or off (0)",
            ),
            Param(
                "Tg",
                "Time constants",
                "s",
                "converter current time constant",
            ),
            Param(
                "Rrpwr",
                "Ramps",
                "pu/s",
                "active current recovery rate after a fault",
            ),
            Param(
                "Brkpt",
                "Low-voltage logic",
                "pu",
                "LVPL breakpoint voltage",
            ),
            Param(
                "Zerox",
                "Low-voltage logic",
                "pu",
                "LVPL zero-crossing voltage",
            ),
            Param(
                "Lvpl1",
                "Low-voltage logic",
                "pu",
                "LVPL current limit at the breakpoint",
            ),
            Param(
                "Volim",
                "Current limits",
                "pu",
                "voltage limit of the high-voltage reactive current management",
            ),
            Param(
                "Lvpnt1",
                "Low-voltage logic",
                "pu",
                "upper voltage of the active current reduction",
            ),
            Param(
                "Lvpnt0",
                "Low-voltage logic",
                "pu",
                "lower voltage of the active current reduction",
            ),
            Param(
                "Iolim",
                "Current limits",
                "pu",
                "reactive current limit of the high-voltage management",
            ),
            Param(
                "Tfltr",
                "Time constants",
                "s",
                "voltage filter time constant of the LVPL",
            ),
            Param(
                "Khv",
                "Current limits",
                "pu",
                "gain of the high-voltage reactive current management",
            ),
            Param("Iqrmax", "Ramps", "pu/s", "reactive current rate up"),
            Param("Iqrmin", "Ramps", "pu/s", "reactive current rate down"),
            Param("Accel", "Other", "", "acceleration factor"),
        ],
    ),
    (
        "REECB1",
        &[
            Param(
                "IREG",
                "Flags",
                "",
                "regulated bus (0: the plant's own terminal)",
            ),
            Param(
                "PFFLAG",
                "Flags",
                "",
                "power factor control (1) or Q control (0)",
            ),
            Param(
                "VFLAG",
                "Flags",
                "",
                "voltage control (1) or Q control (0) of the inner loop",
            ),
            Param(
                "QFLAG",
                "Flags",
                "",
                "closed-loop voltage or Q control (1) or constant pf or Q (0)",
            ),
            Param(
                "PQFLAG",
                "Flags",
                "",
                "P priority (1) or Q priority (0) at the current limit",
            ),
            Param(
                "Vdip",
                "Voltage dip",
                "pu",
                "voltage below which the dip logic acts",
            ),
            Param(
                "Vup",
                "Voltage dip",
                "pu",
                "voltage above which the dip logic acts",
            ),
            Param(
                "Trv",
                "Time constants",
                "s",
                "terminal voltage filter time constant",
            ),
            Param("dbd1", "Voltage dip", "pu", "voltage error deadband, low"),
            Param("dbd2", "Voltage dip", "pu", "voltage error deadband, high"),
            Param(
                "Kqv",
                "Voltage dip",
                "pu",
                "reactive current gain during a voltage dip",
            ),
            Param(
                "Iqh1",
                "Voltage dip",
                "pu",
                "reactive current injection maximum",
            ),
            Param(
                "Iql1",
                "Voltage dip",
                "pu",
                "reactive current injection minimum",
            ),
            Param(
                "Vref0",
                "Reactive control",
                "pu",
                "voltage reference (0: the operating voltage)",
            ),
            Param(
                "Tp",
                "Time constants",
                "s",
                "electrical power filter time constant",
            ),
            Param("Qmax", "Reactive control", "pu", "reactive power maximum"),
            Param("Qmin", "Reactive control", "pu", "reactive power minimum"),
            Param("Vmax", "Reactive control", "pu", "voltage control maximum"),
            Param("Vmin", "Reactive control", "pu", "voltage control minimum"),
            Param(
                "Kqp",
                "Reactive control",
                "pu",
                "reactive power PI, proportional gain",
            ),
            Param(
                "Kqi",
                "Reactive control",
                "pu",
                "reactive power PI, integral gain",
            ),
            Param(
                "Kvp",
                "Reactive control",
                "pu",
                "voltage PI, proportional gain",
            ),
            Param("Kvi", "Reactive control", "pu", "voltage PI, integral gain"),
            Param(
                "Tiq",
                "Time constants",
                "s",
                "reactive current filter time constant",
            ),
            Param("dPmax", "Active control", "pu/s", "active power ramp up"),
            Param("dPmin", "Active control", "pu/s", "active power ramp down"),
            Param("Pmax", "Active control", "pu", "active power maximum"),
            Param("Pmin", "Active control", "pu", "active power minimum"),
            Param("Imax", "Current limits", "pu", "converter current limit"),
            Param(
                "Tpord",
                "Time constants",
                "s",
                "active power order time constant",
            ),
        ],
    ),
    (
        "REPCA1",
        &[
            Param(
                "IREG",
                "Flags",
                "",
                "regulated bus (0: the plant's own terminal)",
            ),
            Param("from", "Flags", "", "monitored branch, from bus (0: none)"),
            Param("to", "Flags", "", "monitored branch, to bus (0: none)"),
            Param("circuit", "Flags", "", "monitored branch circuit"),
            Param(
                "VCFlag",
                "Flags",
                "",
                "line drop compensation (1) or reactive droop (0)",
            ),
            Param(
                "RefFlag",
                "Flags",
                "",
                "voltage control (1) or Q control (0)",
            ),
            Param("Fflag", "Flags", "", "frequency control on (1) or off (0)"),
            Param(
                "Tfltr",
                "Time constants",
                "s",
                "voltage and Q filter time constant",
            ),
            Param(
                "Kp",
                "Volt/var control",
                "pu",
                "volt/var PI, proportional gain",
            ),
            Param("Ki", "Volt/var control", "pu", "volt/var PI, integral gain"),
            Param("Tft", "Time constants", "s", "lead time constant"),
            Param("Tfv", "Time constants", "s", "lag time constant"),
            Param(
                "Vfrz",
                "Volt/var control",
                "pu",
                "voltage below which the volt/var integrator freezes",
            ),
            Param(
                "Rc",
                "Volt/var control",
                "pu",
                "line drop compensation resistance",
            ),
            Param(
                "Xc",
                "Volt/var control",
                "pu",
                "line drop compensation reactance",
            ),
            Param("Kc", "Volt/var control", "pu", "reactive droop gain"),
            Param("emax", "Volt/var control", "pu", "volt/var error maximum"),
            Param("emin", "Volt/var control", "pu", "volt/var error minimum"),
            Param(
                "dbd1",
                "Volt/var control",
                "pu",
                "volt/var error deadband, low",
            ),
            Param(
                "dbd2",
                "Volt/var control",
                "pu",
                "volt/var error deadband, high",
            ),
            Param(
                "Qmax",
                "Volt/var control",
                "pu",
                "plant reactive power maximum",
            ),
            Param(
                "Qmin",
                "Volt/var control",
                "pu",
                "plant reactive power minimum",
            ),
            Param(
                "Kpg",
                "Frequency control",
                "pu",
                "active power PI, proportional gain",
            ),
            Param(
                "Kig",
                "Frequency control",
                "pu",
                "active power PI, integral gain",
            ),
            Param(
                "Tp",
                "Time constants",
                "s",
                "active power filter time constant",
            ),
            Param(
                "fdbd1",
                "Frequency control",
                "pu",
                "frequency deadband, low",
            ),
            Param(
                "fdbd2",
                "Frequency control",
                "pu",
                "frequency deadband, high",
            ),
            Param(
                "femax",
                "Frequency control",
                "pu",
                "frequency error maximum",
            ),
            Param(
                "femin",
                "Frequency control",
                "pu",
                "frequency error minimum",
            ),
            Param(
                "Pmax",
                "Frequency control",
                "pu",
                "plant active power maximum",
            ),
            Param(
                "Pmin",
                "Frequency control",
                "pu",
                "plant active power minimum",
            ),
            Param(
                "Tlag",
                "Time constants",
                "s",
                "active power order lag time constant",
            ),
            Param(
                "Ddn",
                "Frequency control",
                "pu",
                "droop gain, over-frequency",
            ),
            Param(
                "Dup",
                "Frequency control",
                "pu",
                "droop gain, under-frequency",
            ),
        ],
    ),
    // Experimental: models tellegen reads itself (rms/ext_models.rs).
    (
        "ESST1A",
        &[
            Param("UEL", "Inputs", "", "UEL input location (read, not used: limiters enter at the summing point)"),
            Param("VOS", "Inputs", "", "stabilizer input: 1 voltage error, 2 after the regulator"),
            Param("TR", "Measurement", "s", "voltage transducer time constant"),
            Param("VIMAX", "Limits", "pu", "voltage error maximum"),
            Param("VIMIN", "Limits", "pu", "voltage error minimum"),
            Param("TC", "Regulator", "s", "lead-lag 1 lead time constant"),
            Param("TB", "Regulator", "s", "lead-lag 1 lag time constant"),
            Param("TC1", "Regulator", "s", "lead-lag 2 lead"),
            Param("TB1", "Regulator", "s", "lead-lag 2 lag (0 bypasses)"),
            Param("KA", "Regulator", "pu", "regulator gain"),
            Param("TA", "Regulator", "s", "thyristor bridge time constant"),
            Param("VAMAX", "Limits", "pu", "regulator output maximum"),
            Param("VAMIN", "Limits", "pu", "regulator output minimum"),
            Param("VRMAX", "Limits", "pu", "field voltage maximum (times Vt)"),
            Param("VRMIN", "Limits", "pu", "field voltage minimum (times Vt)"),
            Param("KC", "Exciter", "pu", "rectifier loading factor"),
            Param("KF", "Feedback", "pu", "rate feedback gain"),
            Param("TF", "Feedback", "s", "rate feedback time constant"),
            Param("KLR", "Field current limit", "pu", "field current limiter gain"),
            Param("ILR", "Field current limit", "pu", "field current limiter pickup"),
        ],
    ),
    (
        "ESAC6A",
        &[
            Param("TR", "Measurement", "s", "voltage transducer time constant"),
            Param("KA", "Regulator", "pu", "regulator gain"),
            Param("TA", "Regulator", "s", "regulator lag time constant"),
            Param("TK", "Regulator", "s", "regulator lead time constant"),
            Param("TB", "Regulator", "s", "lead-lag lag time constant"),
            Param("TC", "Regulator", "s", "lead-lag lead time constant"),
            Param("VAMAX", "Limits", "pu", "regulator output maximum"),
            Param("VAMIN", "Limits", "pu", "regulator output minimum"),
            Param("VRMAX", "Limits", "pu", "exciter field voltage maximum (times Vt)"),
            Param("VRMIN", "Limits", "pu", "exciter field voltage minimum (times Vt)"),
            Param("TE", "Exciter", "s", "exciter time constant"),
            Param("VFELIM", "Field current limit", "pu", "exciter field current limit"),
            Param("KH", "Field current limit", "pu", "exciter field current limiter gain (0 off)"),
            Param("VHMAX", "Field current limit", "pu", "limiter output maximum"),
            Param("TH", "Field current limit", "s", "limiter lag"),
            Param("TJ", "Field current limit", "s", "limiter lead"),
            Param("KC", "Exciter", "pu", "rectifier loading factor"),
            Param("KD", "Exciter", "pu", "demagnetizing factor"),
            Param("KE", "Exciter", "pu", "exciter constant"),
            Param("E1", "Saturation", "pu", "first saturation point"),
            Param("SE(E1)", "Saturation", "", "saturation at E1"),
            Param("E2", "Saturation", "pu", "second saturation point"),
            Param("SE(E2)", "Saturation", "", "saturation at E2"),
        ],
    ),
    (
        "PSS2B",
        &[
            Param("J1", "Inputs", "", "input 1 signal (1 speed)"),
            Param("K1", "Inputs", "", "input 1 remote bus (0)"),
            Param("J2", "Inputs", "", "input 2 signal (3 electrical power)"),
            Param("K2", "Inputs", "", "input 2 remote bus (0)"),
            Param("M", "Ramp tracking", "", "ramp-tracking filter order M"),
            Param("N", "Ramp tracking", "", "ramp-tracking filter order N"),
            Param("TW1", "Washouts", "s", "input 1 washout 1"),
            Param("TW2", "Washouts", "s", "input 1 washout 2 (0 bypasses)"),
            Param("T6", "Washouts", "s", "input 1 lag (0 bypasses)"),
            Param("TW3", "Washouts", "s", "input 2 washout 1"),
            Param("TW4", "Washouts", "s", "input 2 washout 2 (0 bypasses)"),
            Param("T7", "Washouts", "s", "input 2 integrator lag"),
            Param("KS2", "Gains", "pu", "input 2 gain, T7 / (2H) with H of this machine"),
            Param("KS3", "Gains", "pu", "input 2 summing gain"),
            Param("T8", "Ramp tracking", "s", "ramp-tracking lead"),
            Param("T9", "Ramp tracking", "s", "ramp-tracking lag"),
            Param("KS1", "Gains", "pu", "stabilizer gain"),
            Param("T1", "Phase compensation", "s", "lead-lag 1 lead"),
            Param("T2", "Phase compensation", "s", "lead-lag 1 lag"),
            Param("T3", "Phase compensation", "s", "lead-lag 2 lead"),
            Param("T4", "Phase compensation", "s", "lead-lag 2 lag"),
            Param("T10", "Phase compensation", "s", "lead-lag 3 lead"),
            Param("T11", "Phase compensation", "s", "lead-lag 3 lag"),
            Param("VS1MAX", "Limits", "pu", "input 1 maximum"),
            Param("VS1MIN", "Limits", "pu", "input 1 minimum"),
            Param("VS2MAX", "Limits", "pu", "input 2 maximum"),
            Param("VS2MIN", "Limits", "pu", "input 2 minimum"),
            Param("VSTMAX", "Limits", "pu", "output maximum"),
            Param("VSTMIN", "Limits", "pu", "output minimum"),
        ],
    ),
    (
        "OELIT",
        &[
            Param("IFDRAT", "Field current", "pu", "rated field current (air-gap line)"),
            Param("IPICK", "Field current", "", "heating pickup / IFDRAT"),
            Param("ICONT", "Field current", "", "continuous limit / IFDRAT"),
            Param("ILIM", "Field current", "", "instantaneous limit / IFDRAT"),
            Param("C", "Inverse time", "s", "inverse-time constant: t = C / ((I/Ipick)^2 - 1)"),
            Param("TCOOL", "Inverse time", "s", "cooling time constant"),
            Param("KP", "Limiter", "pu", "proportional gain (pu voltage per pu field current)"),
            Param("KI", "Limiter", "1/s", "integral gain (dead-zone integrator, resets in 1 s)"),
            Param("VOELMIN", "Limiter", "pu", "limiter output minimum"),
        ],
    ),
    (
        "UELPQ",
        &[
            Param("Q0", "Characteristic", "pu", "reactive power limit at P = 0 (MBASE)"),
            Param("KQP", "Characteristic", "pu", "slope of the limit with P"),
            Param("KV", "Characteristic", "", "voltage exponent of the limit (0, 1, 2)"),
            Param("KP", "Limiter", "pu", "proportional gain (pu voltage per pu Q)"),
            Param("KI", "Limiter", "1/s", "integral gain (dead-zone integrator, resets in 1 s)"),
            Param("VUELMAX", "Limiter", "pu", "limiter output maximum"),
        ],
    ),
];

/// The number of parameters of `model` the editor names, if it knows it.
#[cfg(test)]
pub(crate) fn param_count(model: &str) -> Option<usize> {
    names(model).map(<[_]>::len)
}

fn names(model: &str) -> Option<&'static [Param]> {
    let m = model.trim().to_ascii_uppercase();
    SCHEMA.iter().find(|(n, _)| *n == m).map(|(_, p)| *p)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DyrParameter {
    pub name: String,
    /// The token as written in the file.
    pub text: String,
    pub value: f64,
    /// Bucket of the parameter (reactances, time constants, limits, ...).
    pub group: String,
    /// Unit (`pu` on the machine base, `s`, ...).
    pub unit: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DyrRecordView {
    /// First line of the record in the file (1-based); edits name it.
    pub line: usize,
    pub model: String,
    /// `machine`, `exciter`, `governor` or `stabilizer`; for an inverter
    /// record `converter`, `electrical`, `plant` or `inverter` (another
    /// inverter record).
    pub role: String,
    pub parameters: Vec<DyrParameter>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DyrGeneratorView {
    pub bus: usize,
    pub machine_id: String,
    /// Machine base (MVA).
    pub mbase: f64,
    /// Load flow set points and limits (MW, MVAr, pu).
    pub pg: f64,
    pub pmax: f64,
    pub qmax: f64,
    pub qmin: f64,
    pub vg: f64,
    /// Linked records (machine, exciter, governor, stabilizer).
    pub records: Vec<DyrRecordView>,
    /// Inverter-based resource records of the unit (REGCA1, REECB1, ...) by
    /// model name. RMS simulates a REGCA1 + REECB1 + REPCA1 chain as a plant
    /// and keeps other inverter records at the power flow output.
    pub inverter: Vec<String>,
    /// The same records with their parts and named parameters (converter,
    /// electrical control, plant controller, in that order).
    #[serde(default)]
    pub inverter_records: Vec<DyrRecordView>,
}

/// One parameter edit: record line, parameter index (0-based, PSS/E order),
/// new value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DyrEdit {
    pub line: usize,
    pub index: usize,
    pub value: f64,
}

fn record_at(dyr: &DynamicCase, line: usize) -> Option<&DynamicRecord> {
    dyr.records().iter().find(|r| r.line() == line)
}

/// Every in-service generator, its linked records as named parameters
/// (PSS/E order) and its inverter records; a static unit has none.
// @block Read parameters
#[must_use]
pub fn dyr_parameters(network: &BalancedNetwork, dyr: &DynamicCase) -> Vec<DyrGeneratorView> {
    let linking = powerio::link_machines(network, dyr);
    let mut out = Vec::new();
    for m in &linking.machines {
        let g = &network.generators()[m.generator];
        let mut records = Vec::new();
        for (role, line) in [
            ("machine", m.machine.as_ref().map(|l| l.line)),
            ("exciter", m.exciter.as_ref().map(|l| l.line)),
            ("governor", m.governor.as_ref().map(|l| l.line)),
            ("stabilizer", m.stabilizer.as_ref().map(|l| l.line)),
        ] {
            let Some(line) = line else { continue };
            let Some(rec) = record_at(dyr, line) else {
                continue;
            };
            let model = rec.model_name().trim().to_ascii_uppercase();
            let Some(pnames) = names(&model) else {
                continue;
            };
            records.push(DyrRecordView {
                line,
                model,
                role: role.to_string(),
                parameters: parameters(rec, pnames),
            });
        }
        // Experimental: the unit's ESST1A, ESAC6A, PSS2B,
        // OELIT and UELPQ records (powerio does not type them).
        for r in dyr.records().iter().filter(|r| {
            r.bus_id().is_some_and(|b| b as usize == m.bus.0)
                && r.device_id()
                    .unwrap_or("1")
                    .trim()
                    .eq_ignore_ascii_case(m.machine_id.trim())
        }) {
            let model = r.model_name().trim().to_ascii_uppercase();
            let (Some(role), Some(pnames)) = (super::ext_models::ext_role(&model), names(&model)) else {
                continue;
            };
            records.push(DyrRecordView {
                line: r.line(),
                role: role.to_string(),
                parameters: parameters(r, pnames),
                model,
            });
        }
        // The inverter records of the unit, each as one part of the plant
        // (converter, electrical control, plant controller, or another
        // inverter record), in chain order; a model the editor has no names
        // for comes with no parameters.
        let mut inverter_records: Vec<DyrRecordView> = dyr
            .records()
            .iter()
            .filter(|r| {
                r.bus_id().is_some_and(|b| b as usize == m.bus.0)
                    && r.device_id()
                        .unwrap_or("1")
                        .trim()
                        .eq_ignore_ascii_case(m.machine_id.trim())
                    && super::library::is_inverter(r.model_name())
            })
            .map(|r| {
                let model = r.model_name().trim().to_ascii_uppercase();
                DyrRecordView {
                    line: r.line(),
                    role: super::library::inverter_part(&model).to_string(),
                    parameters: names(&model).map_or_else(Vec::new, |p| parameters(r, p)),
                    model,
                }
            })
            .collect();
        let order = |role: &str| {
            super::library::INVERTER_PARTS
                .iter()
                .position(|p| *p == role)
                .unwrap_or(usize::MAX)
        };
        inverter_records.sort_by_key(|r| (order(&r.role), r.line));
        let inverter = linking
            .issues
            .iter()
            .filter_map(|issue| match issue {
                powerio::LinkIssue::Untyped {
                    bus: Some(bus),
                    machine_id: Some(id),
                    error: powerio::ModelError::Unsupported { model_name },
                    ..
                } if *bus == m.bus
                    && id.eq_ignore_ascii_case(&m.machine_id)
                    && super::ext_models::ext_role(model_name).is_none() =>
                {
                    Some(model_name.trim().to_ascii_uppercase())
                }
                _ => None,
            })
            .collect();
        out.push(DyrGeneratorView {
            bus: m.bus.0,
            machine_id: m.machine_id.clone(),
            mbase: m.mbase,
            pg: g.pg,
            pmax: g.pmax,
            qmax: g.qmax,
            qmin: g.qmin,
            vg: g.vg,
            records,
            inverter,
            inverter_records,
        });
    }
    out
}

/// The values of `rec` named by `pnames`, in record order.
fn parameters(rec: &DynamicRecord, pnames: &[Param]) -> Vec<DyrParameter> {
    rec.parameters()
        .iter()
        .zip(pnames.iter())
        .map(|(t, p)| DyrParameter {
            name: p.0.to_string(),
            text: t.trim().to_string(),
            value: t.trim().parse().unwrap_or(f64::NAN),
            group: p.1.to_string(),
            unit: p.2.to_string(),
            description: p.3.to_string(),
        })
        .collect()
}

/// Byte spans of a record's fields: separated by whitespace or commas, a
/// quoted field is one token, `/` ends the record.
fn token_spans(raw: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start: Option<usize> = None;
    let mut quoted = false;
    for (i, c) in raw.char_indices() {
        if quoted {
            if c == '\'' {
                quoted = false;
            }
            continue;
        }
        match c {
            '\'' => {
                start.get_or_insert(i);
                quoted = true;
            }
            '/' => {
                if let Some(s) = start.take() {
                    spans.push((s, i));
                }
                return spans;
            }
            c if c.is_whitespace() || c == ',' => {
                if let Some(s) = start.take() {
                    spans.push((s, i));
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(s) = start {
        spans.push((s, raw.len()));
    }
    spans
}

/// Byte offset of the start of 1-based `line` in `text`.
fn line_offset(text: &str, line: usize) -> Option<usize> {
    if line == 0 {
        return None;
    }
    if line == 1 {
        return Some(0);
    }
    text.match_indices('\n').nth(line - 2).map(|(i, _)| i + 1)
}

/// Shortest decimal text that reads back as `v`.
fn number_text(v: f64) -> String {
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// `text` with the edited parameter tokens replaced.
///
/// # Errors
/// When a line names no record, the index is out of range, the value is not
/// finite, or the record's text cannot be located; the result is read back
/// and must parse to the edited values.
// @block Edit parameters in place
pub fn dyr_edit(text: &str, edits: &[DyrEdit]) -> Result<String, String> {
    let mut out = text.to_string();
    for e in edits {
        if !e.value.is_finite() {
            return Err(format!("line {}: value is not finite", e.line));
        }
        let dyr = powerio::parse_dyr(&out).map_err(|err| format!("DYR: {err}"))?;
        let rec = record_at(&dyr, e.line)
            .ok_or_else(|| format!("no record starts on line {}", e.line))?;
        let n = rec.parameters().len();
        if e.index >= n {
            return Err(format!(
                "line {}: {} has {n} parameters, no index {}",
                e.line,
                rec.model_name(),
                e.index
            ));
        }
        let from =
            line_offset(&out, e.line).ok_or_else(|| format!("line {} is past the end", e.line))?;
        let raw = rec.raw_record();
        let at = out[from..]
            .find(raw)
            .map(|k| from + k)
            .ok_or_else(|| format!("line {}: record text not found", e.line))?;
        let spans = token_spans(raw);
        if spans.len() < n {
            return Err(format!("line {}: record fields not found", e.line));
        }
        let (s, t) = spans[spans.len() - n + e.index];
        if raw[s..t].trim() != rec.parameters()[e.index].trim() {
            return Err(format!(
                "line {}: field {:?} does not match parameter {:?}",
                e.line,
                &raw[s..t],
                rec.parameters()[e.index]
            ));
        }
        out.replace_range(at + s..at + t, &number_text(e.value));
    }
    // Read back: every edited parameter holds its last value.
    let dyr = powerio::parse_dyr(&out).map_err(|err| format!("edited DYR: {err}"))?;
    let mut last = std::collections::BTreeMap::new();
    for e in edits {
        last.insert((e.line, e.index), e.value);
    }
    for ((line, index), value) in last {
        let rec = record_at(&dyr, line).ok_or_else(|| format!("edited DYR lost line {line}"))?;
        let got: f64 = rec.parameters()[index]
            .trim()
            .parse()
            .map_err(|_| format!("line {line}: edited value unreadable"))?;
        if got != value {
            return Err(format!("line {line}: edited value reads back as {got}"));
        }
    }
    Ok(out)
}

/// [`dyr_parameters`] for a stored PowerIO module and DYR text, as JSON.
///
/// # Errors
/// When the inputs do not parse.
pub fn dyr_parameters_json(module_json: &str, dyr_text: &str) -> Result<String, String> {
    let module = crate::ir::balanced_module(crate::ir::deserialize_module(module_json)?)?;
    let dyr = powerio::parse_dyr(dyr_text).map_err(|e| format!("DYR: {e}"))?;
    serde_json::to_string(&dyr_parameters(module.value(), &dyr)).map_err(|e| e.to_string())
}

/// [`dyr_edit`] with the edits as JSON (`[{"line", "index", "value"}]`).
///
/// # Errors
/// As [`dyr_edit`], or when the edits do not parse.
pub fn dyr_edit_json(dyr_text: &str, edits_json: &str) -> Result<String, String> {
    let edits: Vec<DyrEdit> = serde_json::from_str(edits_json).map_err(|e| e.to_string())?;
    dyr_edit(dyr_text, &edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "\n  1 'GENROU' 1   8.0 0.03 0.4 0.05 6.5 0.0 1.8 1.7 0.3 0.55 0.25 0.2\n     0.0 0.0 /\n 1 'SEXS' 1 0.1 10.0 100.0, 0.1 0.0 3.0 /\n";

    #[test]
    fn schema_matches_powerio_arity() {
        for (model, arity) in powerio::SUPPORTED_MODELS {
            assert_eq!(names(model).map(<[_]>::len), Some(*arity), "{model}");
        }
    }

    #[test]
    fn edit_replaces_one_token_only() {
        let dyr = powerio::parse_dyr(TEXT).unwrap();
        let sexs = dyr
            .records()
            .iter()
            .find(|r| r.model_name() == "SEXS")
            .unwrap();
        let genrou = dyr
            .records()
            .iter()
            .find(|r| r.model_name() == "GENROU")
            .unwrap();
        let out = dyr_edit(
            TEXT,
            &[
                DyrEdit {
                    line: sexs.line(),
                    index: 2,
                    value: 200.0,
                },
                DyrEdit {
                    line: genrou.line(),
                    index: 13,
                    value: 0.125,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            out,
            "\n  1 'GENROU' 1   8.0 0.03 0.4 0.05 6.5 0.0 1.8 1.7 0.3 0.55 0.25 0.2\n     0.0 0.125 /\n 1 'SEXS' 1 0.1 10.0 200.0, 0.1 0.0 3.0 /\n"
        );
    }

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/wecc240.rs"
    ));

    /// On the example case: every linked record, one parameter changed,
    /// every other record and parameter as before.
    #[test]
    fn edit_every_linked_record_of_the_examples() {
        let Some(dir) = shipped_wecc240() else { return };
        let raw = powerio::Source::open(dir.join("case.raw")).unwrap();
        let module = powerio::parse(raw).unwrap();
        let powerio::PioValue::BalancedNetwork(net) = module.value() else {
            panic!("not a balanced network")
        };
        let text = std::fs::read_to_string(dir.join("case.dyr")).unwrap();
        let dyr = powerio::parse_dyr(&text).unwrap();
        let views = dyr_parameters(net, &dyr);
        assert!(!views.is_empty());
        // A renewable plant: its three records as parts with named parameters.
        let plant = views
            .iter()
            .find(|g| g.bus == 1032 && g.machine_id.trim() == "S")
            .unwrap();
        let parts: Vec<(&str, &str, usize)> = plant
            .inverter_records
            .iter()
            .map(|r| (r.role.as_str(), r.model.as_str(), r.parameters.len()))
            .collect();
        assert_eq!(
            parts,
            [
                ("converter", "REGCA1", 15),
                ("electrical", "REECB1", 30),
                ("plant", "REPCA1", 34)
            ]
        );
        assert_eq!(plant.inverter_records[1].parameters[28].name, "Imax");
        let mut edited = 0;
        for g in &views {
            for r in g.records.iter().chain(&g.inverter_records) {
                let k = r.parameters.len() - 1;
                let value = r.parameters[k].value + 0.25;
                let out = dyr_edit(
                    &text,
                    &[DyrEdit {
                        line: r.line,
                        index: k,
                        value,
                    }],
                )
                .unwrap();
                let after = powerio::parse_dyr(&out).unwrap();
                assert_eq!(after.records().len(), dyr.records().len());
                for (a, b) in dyr.records().iter().zip(after.records()) {
                    if a.line() == r.line {
                        assert_eq!(b.parameters()[k].parse::<f64>().unwrap(), value);
                        assert_eq!(a.parameters()[..k], b.parameters()[..k]);
                    } else {
                        assert_eq!(a.raw_record(), b.raw_record());
                    }
                }
                edited += 1;
            }
        }
        println!(
            "wecc240: {} generators, {edited} records edited",
            views.len()
        );
    }

    #[test]
    fn edit_rejects_bad_requests() {
        assert!(dyr_edit(
            TEXT,
            &[DyrEdit {
                line: 1,
                index: 0,
                value: 1.0
            }]
        )
        .is_err());
        let line = powerio::parse_dyr(TEXT).unwrap().records()[1].line();
        assert!(dyr_edit(
            TEXT,
            &[DyrEdit {
                line,
                index: 6,
                value: 1.0
            }]
        )
        .is_err());
        assert!(dyr_edit(
            TEXT,
            &[DyrEdit {
                line,
                index: 0,
                value: f64::NAN
            }]
        )
        .is_err());
    }
}
