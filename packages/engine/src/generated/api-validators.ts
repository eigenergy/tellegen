// @ts-nocheck
// Generated from Rust API schemas by generate-api-validators.mjs.
"use strict";
export const calculation = validate331;
const schema20 = {"properties":{"result":true,"settings":{"$ref":"#/$defs/ResolvedSettings"}},"required":["settings","result"],"type":"object"};
const schema21 = {"properties":{"baseline":{"additionalProperties":true,"type":"object"},"calculation":{"$ref":"#/$defs/Calculation"},"effective":{"additionalProperties":true,"type":"object"},"engine_defaults":{"additionalProperties":true,"type":"object"},"overrides":{"additionalProperties":true,"type":"object"},"version":{"format":"uint32","minimum":0,"type":"integer"}},"required":["version","calculation","engine_defaults","baseline","overrides","effective"],"type":"object"};
const schema22 = {"description":"A calculation the common settings API resolves and runs. Each variant's\nJSON name is the wire value every interface accepts.","enum":["mc_ac_pf","dcopf","acpf","dcpf","socwr","acopf"],"type":"string"};

function validate55(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate55.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate55.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((((((data === "mc_ac_pf") || (data === "dcopf")) || (data === "acpf")) || (data === "dcpf")) || (data === "socwr")) || (data === "acopf"))){
validate55.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema22.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate55.errors = vErrors;
return errors === 0;
}
validate55.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate54(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate54.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((data.version === undefined) && (missing0 = "version")) || ((data.calculation === undefined) && (missing0 = "calculation"))) || ((data.engine_defaults === undefined) && (missing0 = "engine_defaults"))) || ((data.baseline === undefined) && (missing0 = "baseline"))) || ((data.overrides === undefined) && (missing0 = "overrides"))) || ((data.effective === undefined) && (missing0 = "effective"))){
validate54.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.baseline !== undefined){
let data0 = data.baseline;
const _errs1 = errors;
if(errors === _errs1){
if(data0 && typeof data0 == "object" && !Array.isArray(data0)){
}
else {
validate54.errors = [{instancePath:instancePath+"/baseline",schemaPath:"#/properties/baseline/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.calculation !== undefined){
const _errs4 = errors;
if(!(validate55(data.calculation, {instancePath:instancePath+"/calculation",parentData:data,parentDataProperty:"calculation",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate55.errors : vErrors.concat(validate55.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.effective !== undefined){
let data2 = data.effective;
const _errs5 = errors;
if(errors === _errs5){
if(data2 && typeof data2 == "object" && !Array.isArray(data2)){
}
else {
validate54.errors = [{instancePath:instancePath+"/effective",schemaPath:"#/properties/effective/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.engine_defaults !== undefined){
let data3 = data.engine_defaults;
const _errs8 = errors;
if(errors === _errs8){
if(data3 && typeof data3 == "object" && !Array.isArray(data3)){
}
else {
validate54.errors = [{instancePath:instancePath+"/engine_defaults",schemaPath:"#/properties/engine_defaults/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.overrides !== undefined){
let data4 = data.overrides;
const _errs11 = errors;
if(errors === _errs11){
if(data4 && typeof data4 == "object" && !Array.isArray(data4)){
}
else {
validate54.errors = [{instancePath:instancePath+"/overrides",schemaPath:"#/properties/overrides/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.version !== undefined){
let data5 = data.version;
const _errs14 = errors;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
validate54.errors = [{instancePath:instancePath+"/version",schemaPath:"#/properties/version/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs14){
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
validate54.errors = [{instancePath:instancePath+"/version",schemaPath:"#/properties/version/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs14 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
else {
validate54.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate54.errors = vErrors;
return errors === 0;
}
validate54.evaluated = {"props":{"baseline":true,"calculation":true,"effective":true,"engine_defaults":true,"overrides":true,"version":true},"dynamicProps":false,"dynamicItems":false};


function validate331(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate331.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.settings === undefined) && (missing0 = "settings")) || ((data.result === undefined) && (missing0 = "result"))){
validate331.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.settings !== undefined){
if(!(validate54(data.settings, {instancePath:instancePath+"/settings",parentData:data,parentDataProperty:"settings",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate54.errors : vErrors.concat(validate54.errors);
errors = vErrors.length;
}
}
}
}
else {
validate331.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate331.errors = vErrors;
return errors === 0;
}
validate331.evaluated = {"props":{"result":true,"settings":true},"dynamicProps":false,"dynamicItems":false};

export const commit = validate333;
const schema23 = {"description":"A committed solve together with its convergence record and sensitivities.","properties":{"iterations":{"anyOf":[{"$ref":"#/$defs/Iterations"},{"type":"null"}]},"sensitivities":{"items":{"$ref":"#/$defs/SensitivityMatrix"},"type":"array"},"solution":{"$ref":"#/$defs/SolveOutput"}},"required":["solution","sensitivities"],"type":"object"};
const schema24 = {"anyOf":[{"description":"Interior-point iterate trace (dcopf / socwr).","items":{"$ref":"#/$defs/SolveIteration"},"type":"array"},{"description":"Newton iteration count and final infinity-norm mismatch (acpf).","properties":{"count":{"format":"uint","minimum":0,"type":"integer"},"residual":{"format":"double","type":"number"}},"required":["count","residual"],"type":"object"},{"description":"Nonlinear-program diagnostics. The primal residual is Tellegen's\nindependent maximum P/Q balance mismatch in MW/MVAr; other residuals\nare separated by family below or reported by POUNCE.","properties":{"base_restoration_calls":{"description":"Restoration entries in the original solve, even when a retry wins.","format":"uint","minimum":0,"type":["integer","null"]},"base_solver_status":{"description":"Status before recovery; absent in older responses.","type":["string","null"]},"model_fingerprint":{"type":"string"},"nlp_iterations":{"format":"uint","minimum":0,"type":"integer"},"primal_residual":{"format":"double","type":"number"},"recovered_by":{"type":["string","null"]},"recovery_attempts":{"items":{"type":"string"},"type":"array"},"residuals":{"anyOf":[{"$ref":"#/$defs/AcOpfResiduals"},{"type":"null"}]},"restoration_calls":{"description":"Restoration entries in the selected solve (not the whole ladder).","format":"uint","minimum":0,"type":["integer","null"]},"solver_constraint_violation":{"format":"double","type":"number"},"solver_kkt_error":{"format":"double","type":"number"},"solver_status":{"type":"string"},"total_nlp_iterations":{"description":"Base plus every retry, including rejected attempts.","format":"uint","minimum":0,"type":["integer","null"]}},"required":["nlp_iterations","primal_residual","solver_status","solver_constraint_violation","solver_kkt_error","model_fingerprint"],"type":"object"}],"description":"The convergence record. OPF paths carry the full interior-point trace (for the\nsolve-card sparkline); the AC power flow carries its Newton count and final\nmismatch. Untagged: the OPF arm serializes to the same bare array the DC OPF\nalways returned."};
const schema25 = {"description":"One interior-point iterate: the iteration index, the primal objective, and\nthe primal and dual residuals. Collected once per Clarabel iteration to draw\nthe convergence plot. Shape matches the frontend `SolveIteration`.","properties":{"inf_du":{"format":"double","type":"number"},"inf_pr":{"format":"double","type":"number"},"iter":{"format":"uint32","minimum":0,"type":"integer"},"objective":{"format":"double","type":"number"}},"required":["iter","objective","inf_pr","inf_du"],"type":"object"};

function validate61(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate61.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((data.iter === undefined) && (missing0 = "iter")) || ((data.objective === undefined) && (missing0 = "objective"))) || ((data.inf_pr === undefined) && (missing0 = "inf_pr"))) || ((data.inf_du === undefined) && (missing0 = "inf_du"))){
validate61.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.inf_du !== undefined){
let data0 = data.inf_du;
const _errs1 = errors;
if(errors === _errs1){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate61.errors = [{instancePath:instancePath+"/inf_du",schemaPath:"#/properties/inf_du/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.inf_pr !== undefined){
let data1 = data.inf_pr;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate61.errors = [{instancePath:instancePath+"/inf_pr",schemaPath:"#/properties/inf_pr/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iter !== undefined){
let data2 = data.iter;
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate61.errors = [{instancePath:instancePath+"/iter",schemaPath:"#/properties/iter/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate61.errors = [{instancePath:instancePath+"/iter",schemaPath:"#/properties/iter/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
let data3 = data.objective;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate61.errors = [{instancePath:instancePath+"/objective",schemaPath:"#/properties/objective/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate61.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate61.errors = vErrors;
return errors === 0;
}
validate61.evaluated = {"props":{"inf_du":true,"inf_pr":true,"iter":true,"objective":true},"dynamicProps":false,"dynamicItems":false};

const schema26 = {"description":"Independent AC OPF checks, with units stated per family. Objective agreement\nis separate from primal feasibility. Bounds are evaluated only when selected.","properties":{"active_balance_mw":{"format":"double","type":"number"},"active_generation_bounds_mw":{"format":"double","type":"number"},"angle_limits_rad":{"format":"double","type":"number"},"objective_relative_error":{"format":"double","type":"number"},"piecewise_epigraph_cost":{"format":"double","type":"number"},"reactive_balance_mvar":{"format":"double","type":"number"},"reactive_generation_bounds_mvar":{"format":"double","type":"number"},"reference_angles_rad":{"format":"double","type":"number"},"thermal_limits_mva":{"format":"double","type":"number"},"voltage_bounds_pu":{"format":"double","type":"number"}},"required":["active_balance_mw","reactive_balance_mvar","voltage_bounds_pu","active_generation_bounds_mw","reactive_generation_bounds_mvar","reference_angles_rad","angle_limits_rad","thermal_limits_mva","piecewise_epigraph_cost","objective_relative_error"],"type":"object"};

function validate63(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate63.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((((((data.active_balance_mw === undefined) && (missing0 = "active_balance_mw")) || ((data.reactive_balance_mvar === undefined) && (missing0 = "reactive_balance_mvar"))) || ((data.voltage_bounds_pu === undefined) && (missing0 = "voltage_bounds_pu"))) || ((data.active_generation_bounds_mw === undefined) && (missing0 = "active_generation_bounds_mw"))) || ((data.reactive_generation_bounds_mvar === undefined) && (missing0 = "reactive_generation_bounds_mvar"))) || ((data.reference_angles_rad === undefined) && (missing0 = "reference_angles_rad"))) || ((data.angle_limits_rad === undefined) && (missing0 = "angle_limits_rad"))) || ((data.thermal_limits_mva === undefined) && (missing0 = "thermal_limits_mva"))) || ((data.piecewise_epigraph_cost === undefined) && (missing0 = "piecewise_epigraph_cost"))) || ((data.objective_relative_error === undefined) && (missing0 = "objective_relative_error"))){
validate63.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.active_balance_mw !== undefined){
let data0 = data.active_balance_mw;
const _errs1 = errors;
if(errors === _errs1){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate63.errors = [{instancePath:instancePath+"/active_balance_mw",schemaPath:"#/properties/active_balance_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.active_generation_bounds_mw !== undefined){
let data1 = data.active_generation_bounds_mw;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate63.errors = [{instancePath:instancePath+"/active_generation_bounds_mw",schemaPath:"#/properties/active_generation_bounds_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.angle_limits_rad !== undefined){
let data2 = data.angle_limits_rad;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate63.errors = [{instancePath:instancePath+"/angle_limits_rad",schemaPath:"#/properties/angle_limits_rad/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective_relative_error !== undefined){
let data3 = data.objective_relative_error;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate63.errors = [{instancePath:instancePath+"/objective_relative_error",schemaPath:"#/properties/objective_relative_error/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.piecewise_epigraph_cost !== undefined){
let data4 = data.piecewise_epigraph_cost;
const _errs9 = errors;
if(errors === _errs9){
if(!((typeof data4 == "number") && (isFinite(data4)))){
validate63.errors = [{instancePath:instancePath+"/piecewise_epigraph_cost",schemaPath:"#/properties/piecewise_epigraph_cost/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.reactive_balance_mvar !== undefined){
let data5 = data.reactive_balance_mvar;
const _errs11 = errors;
if(errors === _errs11){
if(!((typeof data5 == "number") && (isFinite(data5)))){
validate63.errors = [{instancePath:instancePath+"/reactive_balance_mvar",schemaPath:"#/properties/reactive_balance_mvar/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.reactive_generation_bounds_mvar !== undefined){
let data6 = data.reactive_generation_bounds_mvar;
const _errs13 = errors;
if(errors === _errs13){
if(!((typeof data6 == "number") && (isFinite(data6)))){
validate63.errors = [{instancePath:instancePath+"/reactive_generation_bounds_mvar",schemaPath:"#/properties/reactive_generation_bounds_mvar/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.reference_angles_rad !== undefined){
let data7 = data.reference_angles_rad;
const _errs15 = errors;
if(errors === _errs15){
if(!((typeof data7 == "number") && (isFinite(data7)))){
validate63.errors = [{instancePath:instancePath+"/reference_angles_rad",schemaPath:"#/properties/reference_angles_rad/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.thermal_limits_mva !== undefined){
let data8 = data.thermal_limits_mva;
const _errs17 = errors;
if(errors === _errs17){
if(!((typeof data8 == "number") && (isFinite(data8)))){
validate63.errors = [{instancePath:instancePath+"/thermal_limits_mva",schemaPath:"#/properties/thermal_limits_mva/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs17 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_bounds_pu !== undefined){
let data9 = data.voltage_bounds_pu;
const _errs19 = errors;
if(errors === _errs19){
if(!((typeof data9 == "number") && (isFinite(data9)))){
validate63.errors = [{instancePath:instancePath+"/voltage_bounds_pu",schemaPath:"#/properties/voltage_bounds_pu/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs19 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate63.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate63.errors = vErrors;
return errors === 0;
}
validate63.evaluated = {"props":{"active_balance_mw":true,"active_generation_bounds_mw":true,"angle_limits_rad":true,"objective_relative_error":true,"piecewise_epigraph_cost":true,"reactive_balance_mvar":true,"reactive_generation_bounds_mvar":true,"reference_angles_rad":true,"thermal_limits_mva":true,"voltage_bounds_pu":true},"dynamicProps":false,"dynamicItems":false};


function validate60(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate60.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
const _errs1 = errors;
if(errors === _errs1){
if(Array.isArray(data)){
var valid1 = true;
const len0 = data.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate61(data[i0], {instancePath:instancePath+"/" + i0,parentData:data,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate61.errors : vErrors.concat(validate61.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
else {
const err0 = {instancePath,schemaPath:"#/anyOf/0/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
valid0 = valid0 || _valid0;
if(_valid0){
var items0 = true;
}
const _errs4 = errors;
if(errors === _errs4){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.count === undefined) && (missing0 = "count")) || ((data.residual === undefined) && (missing0 = "residual"))){
const err1 = {instancePath,schemaPath:"#/anyOf/1/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
else {
if(data.count !== undefined){
let data1 = data.count;
const _errs6 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
const err2 = {instancePath:instancePath+"/count",schemaPath:"#/anyOf/1/properties/count/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(errors === _errs6){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
const err3 = {instancePath:instancePath+"/count",schemaPath:"#/anyOf/1/properties/count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
}
var valid2 = _errs6 === errors;
}
else {
var valid2 = true;
}
if(valid2){
if(data.residual !== undefined){
let data2 = data.residual;
const _errs8 = errors;
if(errors === _errs8){
if(!((typeof data2 == "number") && (isFinite(data2)))){
const err4 = {instancePath:instancePath+"/residual",schemaPath:"#/anyOf/1/properties/residual/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
var valid2 = _errs8 === errors;
}
else {
var valid2 = true;
}
}
}
}
else {
const err5 = {instancePath,schemaPath:"#/anyOf/1/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
var _valid0 = _errs4 === errors;
valid0 = valid0 || _valid0;
if(_valid0){
var props0 = {};
props0.count = true;
props0.residual = true;
}
const _errs10 = errors;
if(errors === _errs10){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing1;
if(((((((data.nlp_iterations === undefined) && (missing1 = "nlp_iterations")) || ((data.primal_residual === undefined) && (missing1 = "primal_residual"))) || ((data.solver_status === undefined) && (missing1 = "solver_status"))) || ((data.solver_constraint_violation === undefined) && (missing1 = "solver_constraint_violation"))) || ((data.solver_kkt_error === undefined) && (missing1 = "solver_kkt_error"))) || ((data.model_fingerprint === undefined) && (missing1 = "model_fingerprint"))){
const err6 = {instancePath,schemaPath:"#/anyOf/2/required",keyword:"required",params:{missingProperty: missing1},message:"must have required property '"+missing1+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
else {
if(data.base_restoration_calls !== undefined){
let data3 = data.base_restoration_calls;
const _errs12 = errors;
if((!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))) && (data3 !== null)){
const err7 = {instancePath:instancePath+"/base_restoration_calls",schemaPath:"#/anyOf/2/properties/base_restoration_calls/type",keyword:"type",params:{type: schema24.anyOf[2].properties.base_restoration_calls.type},message:"must be integer,null"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
if(errors === _errs12){
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 0 || isNaN(data3)){
const err8 = {instancePath:instancePath+"/base_restoration_calls",schemaPath:"#/anyOf/2/properties/base_restoration_calls/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
}
var valid3 = _errs12 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.base_solver_status !== undefined){
let data4 = data.base_solver_status;
const _errs14 = errors;
if((typeof data4 !== "string") && (data4 !== null)){
const err9 = {instancePath:instancePath+"/base_solver_status",schemaPath:"#/anyOf/2/properties/base_solver_status/type",keyword:"type",params:{type: schema24.anyOf[2].properties.base_solver_status.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
var valid3 = _errs14 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.model_fingerprint !== undefined){
const _errs16 = errors;
if(typeof data.model_fingerprint !== "string"){
const err10 = {instancePath:instancePath+"/model_fingerprint",schemaPath:"#/anyOf/2/properties/model_fingerprint/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
var valid3 = _errs16 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.nlp_iterations !== undefined){
let data6 = data.nlp_iterations;
const _errs18 = errors;
if(!(((typeof data6 == "number") && (!(data6 % 1) && !isNaN(data6))) && (isFinite(data6)))){
const err11 = {instancePath:instancePath+"/nlp_iterations",schemaPath:"#/anyOf/2/properties/nlp_iterations/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
if(errors === _errs18){
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 < 0 || isNaN(data6)){
const err12 = {instancePath:instancePath+"/nlp_iterations",schemaPath:"#/anyOf/2/properties/nlp_iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
}
}
var valid3 = _errs18 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.primal_residual !== undefined){
let data7 = data.primal_residual;
const _errs20 = errors;
if(errors === _errs20){
if(!((typeof data7 == "number") && (isFinite(data7)))){
const err13 = {instancePath:instancePath+"/primal_residual",schemaPath:"#/anyOf/2/properties/primal_residual/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
var valid3 = _errs20 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.recovered_by !== undefined){
let data8 = data.recovered_by;
const _errs22 = errors;
if((typeof data8 !== "string") && (data8 !== null)){
const err14 = {instancePath:instancePath+"/recovered_by",schemaPath:"#/anyOf/2/properties/recovered_by/type",keyword:"type",params:{type: schema24.anyOf[2].properties.recovered_by.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
var valid3 = _errs22 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.recovery_attempts !== undefined){
let data9 = data.recovery_attempts;
const _errs24 = errors;
if(errors === _errs24){
if(Array.isArray(data9)){
var valid4 = true;
const len1 = data9.length;
for(let i1=0; i1<len1; i1++){
const _errs26 = errors;
if(typeof data9[i1] !== "string"){
const err15 = {instancePath:instancePath+"/recovery_attempts/" + i1,schemaPath:"#/anyOf/2/properties/recovery_attempts/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
var valid4 = _errs26 === errors;
if(!valid4){
break;
}
}
}
else {
const err16 = {instancePath:instancePath+"/recovery_attempts",schemaPath:"#/anyOf/2/properties/recovery_attempts/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
}
var valid3 = _errs24 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.residuals !== undefined){
let data11 = data.residuals;
const _errs28 = errors;
const _errs29 = errors;
let valid5 = false;
const _errs30 = errors;
if(!(validate63(data11, {instancePath:instancePath+"/residuals",parentData:data,parentDataProperty:"residuals",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate63.errors : vErrors.concat(validate63.errors);
errors = vErrors.length;
}
var _valid1 = _errs30 === errors;
valid5 = valid5 || _valid1;
if(_valid1){
var props1 = {};
props1.active_balance_mw = true;
props1.active_generation_bounds_mw = true;
props1.angle_limits_rad = true;
props1.objective_relative_error = true;
props1.piecewise_epigraph_cost = true;
props1.reactive_balance_mvar = true;
props1.reactive_generation_bounds_mvar = true;
props1.reference_angles_rad = true;
props1.thermal_limits_mva = true;
props1.voltage_bounds_pu = true;
}
const _errs31 = errors;
if(data11 !== null){
const err17 = {instancePath:instancePath+"/residuals",schemaPath:"#/anyOf/2/properties/residuals/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
var _valid1 = _errs31 === errors;
valid5 = valid5 || _valid1;
if(!valid5){
const err18 = {instancePath:instancePath+"/residuals",schemaPath:"#/anyOf/2/properties/residuals/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
else {
errors = _errs29;
if(vErrors !== null){
if(_errs29){
vErrors.length = _errs29;
}
else {
vErrors = null;
}
}
}
var valid3 = _errs28 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.restoration_calls !== undefined){
let data12 = data.restoration_calls;
const _errs33 = errors;
if((!(((typeof data12 == "number") && (!(data12 % 1) && !isNaN(data12))) && (isFinite(data12)))) && (data12 !== null)){
const err19 = {instancePath:instancePath+"/restoration_calls",schemaPath:"#/anyOf/2/properties/restoration_calls/type",keyword:"type",params:{type: schema24.anyOf[2].properties.restoration_calls.type},message:"must be integer,null"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
if(errors === _errs33){
if((typeof data12 == "number") && (isFinite(data12))){
if(data12 < 0 || isNaN(data12)){
const err20 = {instancePath:instancePath+"/restoration_calls",schemaPath:"#/anyOf/2/properties/restoration_calls/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
}
}
var valid3 = _errs33 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.solver_constraint_violation !== undefined){
let data13 = data.solver_constraint_violation;
const _errs35 = errors;
if(errors === _errs35){
if(!((typeof data13 == "number") && (isFinite(data13)))){
const err21 = {instancePath:instancePath+"/solver_constraint_violation",schemaPath:"#/anyOf/2/properties/solver_constraint_violation/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
}
var valid3 = _errs35 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.solver_kkt_error !== undefined){
let data14 = data.solver_kkt_error;
const _errs37 = errors;
if(errors === _errs37){
if(!((typeof data14 == "number") && (isFinite(data14)))){
const err22 = {instancePath:instancePath+"/solver_kkt_error",schemaPath:"#/anyOf/2/properties/solver_kkt_error/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
}
var valid3 = _errs37 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.solver_status !== undefined){
const _errs39 = errors;
if(typeof data.solver_status !== "string"){
const err23 = {instancePath:instancePath+"/solver_status",schemaPath:"#/anyOf/2/properties/solver_status/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
var valid3 = _errs39 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.total_nlp_iterations !== undefined){
let data16 = data.total_nlp_iterations;
const _errs41 = errors;
if((!(((typeof data16 == "number") && (!(data16 % 1) && !isNaN(data16))) && (isFinite(data16)))) && (data16 !== null)){
const err24 = {instancePath:instancePath+"/total_nlp_iterations",schemaPath:"#/anyOf/2/properties/total_nlp_iterations/type",keyword:"type",params:{type: schema24.anyOf[2].properties.total_nlp_iterations.type},message:"must be integer,null"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
if(errors === _errs41){
if((typeof data16 == "number") && (isFinite(data16))){
if(data16 < 0 || isNaN(data16)){
const err25 = {instancePath:instancePath+"/total_nlp_iterations",schemaPath:"#/anyOf/2/properties/total_nlp_iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
}
}
var valid3 = _errs41 === errors;
}
else {
var valid3 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
const err26 = {instancePath,schemaPath:"#/anyOf/2/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
}
var _valid0 = _errs10 === errors;
valid0 = valid0 || _valid0;
if(_valid0){
if(props0 !== true){
props0 = props0 || {};
props0.base_restoration_calls = true;
props0.base_solver_status = true;
props0.model_fingerprint = true;
props0.nlp_iterations = true;
props0.primal_residual = true;
props0.recovered_by = true;
props0.recovery_attempts = true;
props0.residuals = true;
props0.restoration_calls = true;
props0.solver_constraint_violation = true;
props0.solver_kkt_error = true;
props0.solver_status = true;
props0.total_nlp_iterations = true;
}
}
if(!valid0){
const err27 = {instancePath,schemaPath:"#/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
validate60.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate60.errors = vErrors;
evaluated0.props = props0;
evaluated0.items = items0;
return errors === 0;
}
validate60.evaluated = {"dynamicProps":true,"dynamicItems":true};

const schema27 = {"description":"A self-describing sensitivity result: `values[r][c] = d(rows[r])/d(cols[c])`,\nwith row and column metadata naming each quantity and its source element, and the\nserved-unit label.","properties":{"cols":{"items":{"$ref":"#/$defs/ColMeta"},"type":"array"},"rows":{"items":{"$ref":"#/$defs/RowMeta"},"type":"array"},"units":{"type":"string"},"values":{"description":"`values[r][c] = d(operand_r)/d(parameter_c)`, in the units named by `units`. The\nengine produces per-unit values; the api edge rescales to served units.","items":{"items":{"format":"double","type":"number"},"type":"array"},"type":"array"}},"required":["values","rows","cols","units"],"type":"object"};
const schema28 = {"description":"Identity and quantity metadata for one column of a [`SensitivityMatrix`].","properties":{"element":{"$ref":"#/$defs/ElementId"},"index":{"description":"Dense index of the element this column varies.","format":"uint","minimum":0,"type":"integer"},"parameter":{"$ref":"#/$defs/Parameter"}},"required":["parameter","element","index"],"type":"object"};
const schema29 = {"description":"A source-id reference for a dense row or column, so a [`SensitivityMatrix`]\nself-describes and the api serializes it without re-deriving id maps.","oneOf":[{"additionalProperties":false,"properties":{"Bus":{"format":"uint","minimum":0,"type":"integer"}},"required":["Bus"],"type":"object"},{"additionalProperties":false,"properties":{"Branch":{"format":"uint","minimum":0,"type":"integer"}},"required":["Branch"],"type":"object"},{"additionalProperties":false,"properties":{"Generator":{"format":"uint","minimum":0,"type":"integer"}},"required":["Generator"],"type":"object"},{"additionalProperties":false,"description":"A caller-supplied linear constraint, by its position in the request.","properties":{"Constraint":{"format":"uint","minimum":0,"type":"integer"}},"required":["Constraint"],"type":"object"}]};

function validate68(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate68.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(errors === _errs1){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((data.Bus === undefined) && (missing0 = "Bus")){
const err0 = {instancePath,schemaPath:"#/oneOf/0/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
else {
const _errs3 = errors;
for(const key0 in data){
if(!(key0 === "Bus")){
const err1 = {instancePath,schemaPath:"#/oneOf/0/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
break;
}
}
if(_errs3 === errors){
if(data.Bus !== undefined){
let data0 = data.Bus;
const _errs4 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
const err2 = {instancePath:instancePath+"/Bus",schemaPath:"#/oneOf/0/properties/Bus/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(errors === _errs4){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
const err3 = {instancePath:instancePath+"/Bus",schemaPath:"#/oneOf/0/properties/Bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
}
}
}
}
}
else {
const err4 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
var props0 = true;
}
const _errs6 = errors;
if(errors === _errs6){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing1;
if((data.Branch === undefined) && (missing1 = "Branch")){
const err5 = {instancePath,schemaPath:"#/oneOf/1/required",keyword:"required",params:{missingProperty: missing1},message:"must have required property '"+missing1+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
else {
const _errs8 = errors;
for(const key1 in data){
if(!(key1 === "Branch")){
const err6 = {instancePath,schemaPath:"#/oneOf/1/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
break;
}
}
if(_errs8 === errors){
if(data.Branch !== undefined){
let data1 = data.Branch;
const _errs9 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
const err7 = {instancePath:instancePath+"/Branch",schemaPath:"#/oneOf/1/properties/Branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
if(errors === _errs9){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
const err8 = {instancePath:instancePath+"/Branch",schemaPath:"#/oneOf/1/properties/Branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
}
}
}
}
}
else {
const err9 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
var _valid0 = _errs6 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
if(props0 !== true){
props0 = true;
}
}
const _errs11 = errors;
if(errors === _errs11){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing2;
if((data.Generator === undefined) && (missing2 = "Generator")){
const err10 = {instancePath,schemaPath:"#/oneOf/2/required",keyword:"required",params:{missingProperty: missing2},message:"must have required property '"+missing2+"'"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
else {
const _errs13 = errors;
for(const key2 in data){
if(!(key2 === "Generator")){
const err11 = {instancePath,schemaPath:"#/oneOf/2/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
break;
}
}
if(_errs13 === errors){
if(data.Generator !== undefined){
let data2 = data.Generator;
const _errs14 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
const err12 = {instancePath:instancePath+"/Generator",schemaPath:"#/oneOf/2/properties/Generator/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
if(errors === _errs14){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
const err13 = {instancePath:instancePath+"/Generator",schemaPath:"#/oneOf/2/properties/Generator/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
}
}
}
}
}
else {
const err14 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
}
var _valid0 = _errs11 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
if(props0 !== true){
props0 = true;
}
}
const _errs16 = errors;
if(errors === _errs16){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing3;
if((data.Constraint === undefined) && (missing3 = "Constraint")){
const err15 = {instancePath,schemaPath:"#/oneOf/3/required",keyword:"required",params:{missingProperty: missing3},message:"must have required property '"+missing3+"'"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
else {
const _errs18 = errors;
for(const key3 in data){
if(!(key3 === "Constraint")){
const err16 = {instancePath,schemaPath:"#/oneOf/3/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key3},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
break;
}
}
if(_errs18 === errors){
if(data.Constraint !== undefined){
let data3 = data.Constraint;
const _errs19 = errors;
if(!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))){
const err17 = {instancePath:instancePath+"/Constraint",schemaPath:"#/oneOf/3/properties/Constraint/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
if(errors === _errs19){
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 0 || isNaN(data3)){
const err18 = {instancePath:instancePath+"/Constraint",schemaPath:"#/oneOf/3/properties/Constraint/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
}
}
}
}
}
}
else {
const err19 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
}
var _valid0 = _errs16 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
if(props0 !== true){
props0 = true;
}
}
}
}
}
if(!valid0){
const err20 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
validate68.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate68.errors = vErrors;
evaluated0.props = props0;
return errors === 0;
}
validate68.evaluated = {"dynamicProps":true,"dynamicItems":false};

const schema30 = {"description":"What a sensitivity is taken *with respect to* — the parameter whose hand-derived\n`dK/dp` column drives the solve.","oneOf":[{"additionalProperties":false,"description":"Bus active/reactive demand `pd` / `qd`, per bus.","properties":{"Demand":{"$ref":"#/$defs/Power"}},"required":["Demand"],"type":"object"},{"additionalProperties":false,"description":"Generation cost coefficient, per generator.","properties":{"Cost":{"$ref":"#/$defs/CostTerm"}},"required":["Cost"],"type":"object"},{"const":"LineLimit","description":"DC thermal limit `fmax` / AC-conic apparent-power `|S|` limit, per branch.","type":"string"},{"additionalProperties":false,"description":"Branch series admittance `g` / `b`, per branch.","properties":{"SeriesAdmittance":{"$ref":"#/$defs/GB"}},"required":["SeriesAdmittance"],"type":"object"},{"additionalProperties":false,"description":"Bus shunt admittance `gs` / `bs`, per bus.","properties":{"ShuntAdmittance":{"$ref":"#/$defs/GB"}},"required":["ShuntAdmittance"],"type":"object"},{"additionalProperties":false,"description":"Voltage-magnitude bound right-hand side (the `w = |V|²` bound), per bus.","properties":{"VoltageBound":{"$ref":"#/$defs/Bound"}},"required":["VoltageBound"],"type":"object"},{"additionalProperties":false,"description":"Generator output bound right-hand side (`pmax` / `pmin` / `qmax` / `qmin`),\nper generator.","properties":{"GenBound":{"properties":{"bound":{"$ref":"#/$defs/Bound"},"power":{"$ref":"#/$defs/Power"}},"required":["power","bound"],"type":"object"}},"required":["GenBound"],"type":"object"},{"additionalProperties":false,"description":"Transformer tap ratio / phase shift, per branch.","properties":{"Transformer":{"$ref":"#/$defs/TapKind"}},"required":["Transformer"],"type":"object"},{"const":"Switching","description":"Branch switching state `sw` in `[0, 1]`, per branch.","type":"string"},{"const":"ConstraintLimit","description":"A shift of both limits of a caller-supplied linear constraint (the\nfixed value of an equality), per constraint in request order.","type":"string"}]};
const schema31 = {"description":"Active or reactive power. The orthogonal P/Q sub-axis shared by prices,\ndispatch, flows, and demand.","enum":["Active","Reactive"],"type":"string"};

function validate71(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate71.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate71.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "Active") || (data === "Reactive"))){
validate71.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema31.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate71.errors = vErrors;
return errors === 0;
}
validate71.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema32 = {"description":"Which generation-cost coefficient: the quadratic `cq` or the linear `cl`.","enum":["Quadratic","Linear"],"type":"string"};

function validate73(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate73.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate73.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "Quadratic") || (data === "Linear"))){
validate73.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema32.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate73.errors = vErrors;
return errors === 0;
}
validate73.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema33 = {"description":"Conductance `g` or susceptance `b`, the real/imaginary parts of an admittance.","enum":["Conductance","Susceptance"],"type":"string"};

function validate75(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate75.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate75.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "Conductance") || (data === "Susceptance"))){
validate75.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema33.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate75.errors = vErrors;
return errors === 0;
}
validate75.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema34 = {"description":"A min or max bound, e.g. the lower/upper voltage-magnitude limit.","enum":["Min","Max"],"type":"string"};

function validate78(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate78.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate78.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "Min") || (data === "Max"))){
validate78.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema34.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate78.errors = vErrors;
return errors === 0;
}
validate78.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema35 = {"description":"A transformer control: the tap ratio or the phase-shift angle.","enum":["Ratio","PhaseShift"],"type":"string"};

function validate82(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate82.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate82.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "Ratio") || (data === "PhaseShift"))){
validate82.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema35.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate82.errors = vErrors;
return errors === 0;
}
validate82.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate70(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate70.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(errors === _errs1){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((data.Demand === undefined) && (missing0 = "Demand")){
const err0 = {instancePath,schemaPath:"#/oneOf/0/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
else {
const _errs3 = errors;
for(const key0 in data){
if(!(key0 === "Demand")){
const err1 = {instancePath,schemaPath:"#/oneOf/0/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
break;
}
}
if(_errs3 === errors){
if(data.Demand !== undefined){
if(!(validate71(data.Demand, {instancePath:instancePath+"/Demand",parentData:data,parentDataProperty:"Demand",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate71.errors : vErrors.concat(validate71.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err2 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
var props0 = true;
}
const _errs5 = errors;
if(errors === _errs5){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing1;
if((data.Cost === undefined) && (missing1 = "Cost")){
const err3 = {instancePath,schemaPath:"#/oneOf/1/required",keyword:"required",params:{missingProperty: missing1},message:"must have required property '"+missing1+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
else {
const _errs7 = errors;
for(const key1 in data){
if(!(key1 === "Cost")){
const err4 = {instancePath,schemaPath:"#/oneOf/1/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
break;
}
}
if(_errs7 === errors){
if(data.Cost !== undefined){
if(!(validate73(data.Cost, {instancePath:instancePath+"/Cost",parentData:data,parentDataProperty:"Cost",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate73.errors : vErrors.concat(validate73.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err5 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
if(props0 !== true){
props0 = true;
}
}
const _errs9 = errors;
if(typeof data !== "string"){
const err6 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if("LineLimit" !== data){
const err7 = {instancePath,schemaPath:"#/oneOf/2/const",keyword:"const",params:{allowedValue: "LineLimit"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
var _valid0 = _errs9 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
}
const _errs11 = errors;
if(errors === _errs11){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing2;
if((data.SeriesAdmittance === undefined) && (missing2 = "SeriesAdmittance")){
const err8 = {instancePath,schemaPath:"#/oneOf/3/required",keyword:"required",params:{missingProperty: missing2},message:"must have required property '"+missing2+"'"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
else {
const _errs13 = errors;
for(const key2 in data){
if(!(key2 === "SeriesAdmittance")){
const err9 = {instancePath,schemaPath:"#/oneOf/3/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
break;
}
}
if(_errs13 === errors){
if(data.SeriesAdmittance !== undefined){
if(!(validate75(data.SeriesAdmittance, {instancePath:instancePath+"/SeriesAdmittance",parentData:data,parentDataProperty:"SeriesAdmittance",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate75.errors : vErrors.concat(validate75.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err10 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
var _valid0 = _errs11 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
if(props0 !== true){
props0 = true;
}
}
const _errs15 = errors;
if(errors === _errs15){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing3;
if((data.ShuntAdmittance === undefined) && (missing3 = "ShuntAdmittance")){
const err11 = {instancePath,schemaPath:"#/oneOf/4/required",keyword:"required",params:{missingProperty: missing3},message:"must have required property '"+missing3+"'"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
else {
const _errs17 = errors;
for(const key3 in data){
if(!(key3 === "ShuntAdmittance")){
const err12 = {instancePath,schemaPath:"#/oneOf/4/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key3},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
break;
}
}
if(_errs17 === errors){
if(data.ShuntAdmittance !== undefined){
if(!(validate75(data.ShuntAdmittance, {instancePath:instancePath+"/ShuntAdmittance",parentData:data,parentDataProperty:"ShuntAdmittance",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate75.errors : vErrors.concat(validate75.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err13 = {instancePath,schemaPath:"#/oneOf/4/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
var _valid0 = _errs15 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 4];
}
else {
if(_valid0){
valid0 = true;
passing0 = 4;
if(props0 !== true){
props0 = true;
}
}
const _errs19 = errors;
if(errors === _errs19){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing4;
if((data.VoltageBound === undefined) && (missing4 = "VoltageBound")){
const err14 = {instancePath,schemaPath:"#/oneOf/5/required",keyword:"required",params:{missingProperty: missing4},message:"must have required property '"+missing4+"'"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
else {
const _errs21 = errors;
for(const key4 in data){
if(!(key4 === "VoltageBound")){
const err15 = {instancePath,schemaPath:"#/oneOf/5/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key4},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
break;
}
}
if(_errs21 === errors){
if(data.VoltageBound !== undefined){
if(!(validate78(data.VoltageBound, {instancePath:instancePath+"/VoltageBound",parentData:data,parentDataProperty:"VoltageBound",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate78.errors : vErrors.concat(validate78.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err16 = {instancePath,schemaPath:"#/oneOf/5/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
}
var _valid0 = _errs19 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 5];
}
else {
if(_valid0){
valid0 = true;
passing0 = 5;
if(props0 !== true){
props0 = true;
}
}
const _errs23 = errors;
if(errors === _errs23){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing5;
if((data.GenBound === undefined) && (missing5 = "GenBound")){
const err17 = {instancePath,schemaPath:"#/oneOf/6/required",keyword:"required",params:{missingProperty: missing5},message:"must have required property '"+missing5+"'"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
else {
const _errs25 = errors;
for(const key5 in data){
if(!(key5 === "GenBound")){
const err18 = {instancePath,schemaPath:"#/oneOf/6/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key5},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
break;
}
}
if(_errs25 === errors){
if(data.GenBound !== undefined){
let data5 = data.GenBound;
const _errs26 = errors;
if(errors === _errs26){
if(data5 && typeof data5 == "object" && !Array.isArray(data5)){
let missing6;
if(((data5.power === undefined) && (missing6 = "power")) || ((data5.bound === undefined) && (missing6 = "bound"))){
const err19 = {instancePath:instancePath+"/GenBound",schemaPath:"#/oneOf/6/properties/GenBound/required",keyword:"required",params:{missingProperty: missing6},message:"must have required property '"+missing6+"'"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
else {
if(data5.bound !== undefined){
const _errs28 = errors;
if(!(validate78(data5.bound, {instancePath:instancePath+"/GenBound/bound",parentData:data5,parentDataProperty:"bound",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate78.errors : vErrors.concat(validate78.errors);
errors = vErrors.length;
}
var valid7 = _errs28 === errors;
}
else {
var valid7 = true;
}
if(valid7){
if(data5.power !== undefined){
const _errs29 = errors;
if(!(validate71(data5.power, {instancePath:instancePath+"/GenBound/power",parentData:data5,parentDataProperty:"power",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate71.errors : vErrors.concat(validate71.errors);
errors = vErrors.length;
}
var valid7 = _errs29 === errors;
}
else {
var valid7 = true;
}
}
}
}
else {
const err20 = {instancePath:instancePath+"/GenBound",schemaPath:"#/oneOf/6/properties/GenBound/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
}
}
}
}
}
else {
const err21 = {instancePath,schemaPath:"#/oneOf/6/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
}
var _valid0 = _errs23 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 6];
}
else {
if(_valid0){
valid0 = true;
passing0 = 6;
if(props0 !== true){
props0 = true;
}
}
const _errs30 = errors;
if(errors === _errs30){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing7;
if((data.Transformer === undefined) && (missing7 = "Transformer")){
const err22 = {instancePath,schemaPath:"#/oneOf/7/required",keyword:"required",params:{missingProperty: missing7},message:"must have required property '"+missing7+"'"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
else {
const _errs32 = errors;
for(const key6 in data){
if(!(key6 === "Transformer")){
const err23 = {instancePath,schemaPath:"#/oneOf/7/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key6},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
break;
}
}
if(_errs32 === errors){
if(data.Transformer !== undefined){
if(!(validate82(data.Transformer, {instancePath:instancePath+"/Transformer",parentData:data,parentDataProperty:"Transformer",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate82.errors : vErrors.concat(validate82.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err24 = {instancePath,schemaPath:"#/oneOf/7/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
}
var _valid0 = _errs30 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 7];
}
else {
if(_valid0){
valid0 = true;
passing0 = 7;
if(props0 !== true){
props0 = true;
}
}
const _errs34 = errors;
if(typeof data !== "string"){
const err25 = {instancePath,schemaPath:"#/oneOf/8/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
if("Switching" !== data){
const err26 = {instancePath,schemaPath:"#/oneOf/8/const",keyword:"const",params:{allowedValue: "Switching"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
var _valid0 = _errs34 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 8];
}
else {
if(_valid0){
valid0 = true;
passing0 = 8;
}
const _errs36 = errors;
if(typeof data !== "string"){
const err27 = {instancePath,schemaPath:"#/oneOf/9/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
if("ConstraintLimit" !== data){
const err28 = {instancePath,schemaPath:"#/oneOf/9/const",keyword:"const",params:{allowedValue: "ConstraintLimit"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
var _valid0 = _errs36 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 9];
}
else {
if(_valid0){
valid0 = true;
passing0 = 9;
}
}
}
}
}
}
}
}
}
}
if(!valid0){
const err29 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
validate70.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate70.errors = vErrors;
evaluated0.props = props0;
return errors === 0;
}
validate70.evaluated = {"dynamicProps":true,"dynamicItems":false};


function validate67(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate67.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.parameter === undefined) && (missing0 = "parameter")) || ((data.element === undefined) && (missing0 = "element"))) || ((data.index === undefined) && (missing0 = "index"))){
validate67.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.element !== undefined){
const _errs1 = errors;
if(!(validate68(data.element, {instancePath:instancePath+"/element",parentData:data,parentDataProperty:"element",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate68.errors : vErrors.concat(validate68.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.index !== undefined){
let data1 = data.index;
const _errs2 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate67.errors = [{instancePath:instancePath+"/index",schemaPath:"#/properties/index/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs2){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate67.errors = [{instancePath:instancePath+"/index",schemaPath:"#/properties/index/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.parameter !== undefined){
const _errs4 = errors;
if(!(validate70(data.parameter, {instancePath:instancePath+"/parameter",parentData:data,parentDataProperty:"parameter",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate70.errors : vErrors.concat(validate70.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate67.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate67.errors = vErrors;
return errors === 0;
}
validate67.evaluated = {"props":{"element":true,"index":true,"parameter":true},"dynamicProps":false,"dynamicItems":false};

const schema36 = {"description":"Identity and quantity metadata for one row of a [`SensitivityMatrix`].","properties":{"element":{"$ref":"#/$defs/ElementId"},"index":{"description":"Dense index of the element this row reports.","format":"uint","minimum":0,"type":"integer"},"operand":{"$ref":"#/$defs/Operand"}},"required":["operand","element","index"],"type":"object"};
const schema37 = {"description":"What a sensitivity is taken *of*, named for the physical power quantity rather\nthan a formulation's internal variable. The orthogonal sub-axes (active/reactive,\nfrom/to, voltage representation) are visible to the type system; each formulation\nmaps a request to its own KKT rows or returns [`SensError::Unsupported`].\n\nThe dialects fold into one vocabulary: DC `Angle` is `Voltage(Angle)`, the conic\nsquared magnitude `w` is `Voltage(Squared)`, the nodal price is `Price(Active)`.","oneOf":[{"additionalProperties":false,"description":"Nodal-balance dual (the LMP for active power, the reactive price for\nreactive), per bus.","properties":{"Price":{"$ref":"#/$defs/Power"}},"required":["Price"],"type":"object"},{"additionalProperties":false,"description":"Generator dispatch `pg` / `qg`, per generator.","properties":{"Dispatch":{"$ref":"#/$defs/Power"}},"required":["Dispatch"],"type":"object"},{"additionalProperties":false,"description":"Branch active/reactive flow at the from/to end, per branch.","properties":{"Flow":{"properties":{"end":{"$ref":"#/$defs/End"},"power":{"$ref":"#/$defs/Power"}},"required":["power","end"],"type":"object"}},"required":["Flow"],"type":"object"},{"additionalProperties":false,"description":"Bus voltage in the formulation's representation (`vm` / `va` / `w` / `wr` /\n`wi`), per bus.","properties":{"Voltage":{"$ref":"#/$defs/VoltageKind"}},"required":["Voltage"],"type":"object"}]};
const schema38 = {"description":"Which end of a branch a flow is measured at.","enum":["From","To"],"type":"string"};

function validate91(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate91.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate91.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "From") || (data === "To"))){
validate91.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema38.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate91.errors = vErrors;
return errors === 0;
}
validate91.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema39 = {"description":"How a voltage operand is represented: polar (`Magnitude`, `Angle`) for the AC\npower flow, or the W-space lift (`Squared` = `|V|²`, `ProductReal` = `Re(Vi Vj*)`,\n`ProductImag` = `Im(Vi Vj*)`) for the conic relaxation.","enum":["Magnitude","Angle","Squared","ProductReal","ProductImag"],"type":"string"};

function validate94(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate94.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate94.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!(((((data === "Magnitude") || (data === "Angle")) || (data === "Squared")) || (data === "ProductReal")) || (data === "ProductImag"))){
validate94.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema39.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate94.errors = vErrors;
return errors === 0;
}
validate94.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate88(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate88.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(errors === _errs1){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((data.Price === undefined) && (missing0 = "Price")){
const err0 = {instancePath,schemaPath:"#/oneOf/0/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
else {
const _errs3 = errors;
for(const key0 in data){
if(!(key0 === "Price")){
const err1 = {instancePath,schemaPath:"#/oneOf/0/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
break;
}
}
if(_errs3 === errors){
if(data.Price !== undefined){
if(!(validate71(data.Price, {instancePath:instancePath+"/Price",parentData:data,parentDataProperty:"Price",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate71.errors : vErrors.concat(validate71.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err2 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
var props0 = true;
}
const _errs5 = errors;
if(errors === _errs5){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing1;
if((data.Dispatch === undefined) && (missing1 = "Dispatch")){
const err3 = {instancePath,schemaPath:"#/oneOf/1/required",keyword:"required",params:{missingProperty: missing1},message:"must have required property '"+missing1+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
else {
const _errs7 = errors;
for(const key1 in data){
if(!(key1 === "Dispatch")){
const err4 = {instancePath,schemaPath:"#/oneOf/1/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
break;
}
}
if(_errs7 === errors){
if(data.Dispatch !== undefined){
if(!(validate71(data.Dispatch, {instancePath:instancePath+"/Dispatch",parentData:data,parentDataProperty:"Dispatch",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate71.errors : vErrors.concat(validate71.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err5 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
if(props0 !== true){
props0 = true;
}
}
const _errs9 = errors;
if(errors === _errs9){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing2;
if((data.Flow === undefined) && (missing2 = "Flow")){
const err6 = {instancePath,schemaPath:"#/oneOf/2/required",keyword:"required",params:{missingProperty: missing2},message:"must have required property '"+missing2+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
else {
const _errs11 = errors;
for(const key2 in data){
if(!(key2 === "Flow")){
const err7 = {instancePath,schemaPath:"#/oneOf/2/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
break;
}
}
if(_errs11 === errors){
if(data.Flow !== undefined){
let data2 = data.Flow;
const _errs12 = errors;
if(errors === _errs12){
if(data2 && typeof data2 == "object" && !Array.isArray(data2)){
let missing3;
if(((data2.power === undefined) && (missing3 = "power")) || ((data2.end === undefined) && (missing3 = "end"))){
const err8 = {instancePath:instancePath+"/Flow",schemaPath:"#/oneOf/2/properties/Flow/required",keyword:"required",params:{missingProperty: missing3},message:"must have required property '"+missing3+"'"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
else {
if(data2.end !== undefined){
const _errs14 = errors;
if(!(validate91(data2.end, {instancePath:instancePath+"/Flow/end",parentData:data2,parentDataProperty:"end",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate91.errors : vErrors.concat(validate91.errors);
errors = vErrors.length;
}
var valid4 = _errs14 === errors;
}
else {
var valid4 = true;
}
if(valid4){
if(data2.power !== undefined){
const _errs15 = errors;
if(!(validate71(data2.power, {instancePath:instancePath+"/Flow/power",parentData:data2,parentDataProperty:"power",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate71.errors : vErrors.concat(validate71.errors);
errors = vErrors.length;
}
var valid4 = _errs15 === errors;
}
else {
var valid4 = true;
}
}
}
}
else {
const err9 = {instancePath:instancePath+"/Flow",schemaPath:"#/oneOf/2/properties/Flow/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
}
}
}
}
else {
const err10 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
var _valid0 = _errs9 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
if(props0 !== true){
props0 = true;
}
}
const _errs16 = errors;
if(errors === _errs16){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing4;
if((data.Voltage === undefined) && (missing4 = "Voltage")){
const err11 = {instancePath,schemaPath:"#/oneOf/3/required",keyword:"required",params:{missingProperty: missing4},message:"must have required property '"+missing4+"'"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
else {
const _errs18 = errors;
for(const key3 in data){
if(!(key3 === "Voltage")){
const err12 = {instancePath,schemaPath:"#/oneOf/3/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key3},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
break;
}
}
if(_errs18 === errors){
if(data.Voltage !== undefined){
if(!(validate94(data.Voltage, {instancePath:instancePath+"/Voltage",parentData:data,parentDataProperty:"Voltage",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate94.errors : vErrors.concat(validate94.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err13 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
var _valid0 = _errs16 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
if(props0 !== true){
props0 = true;
}
}
}
}
}
if(!valid0){
const err14 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
validate88.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate88.errors = vErrors;
evaluated0.props = props0;
return errors === 0;
}
validate88.evaluated = {"dynamicProps":true,"dynamicItems":false};


function validate86(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate86.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.operand === undefined) && (missing0 = "operand")) || ((data.element === undefined) && (missing0 = "element"))) || ((data.index === undefined) && (missing0 = "index"))){
validate86.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.element !== undefined){
const _errs1 = errors;
if(!(validate68(data.element, {instancePath:instancePath+"/element",parentData:data,parentDataProperty:"element",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate68.errors : vErrors.concat(validate68.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.index !== undefined){
let data1 = data.index;
const _errs2 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate86.errors = [{instancePath:instancePath+"/index",schemaPath:"#/properties/index/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs2){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate86.errors = [{instancePath:instancePath+"/index",schemaPath:"#/properties/index/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.operand !== undefined){
const _errs4 = errors;
if(!(validate88(data.operand, {instancePath:instancePath+"/operand",parentData:data,parentDataProperty:"operand",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate88.errors : vErrors.concat(validate88.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate86.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate86.errors = vErrors;
return errors === 0;
}
validate86.evaluated = {"props":{"element":true,"index":true,"operand":true},"dynamicProps":false,"dynamicItems":false};


function validate66(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate66.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((data.values === undefined) && (missing0 = "values")) || ((data.rows === undefined) && (missing0 = "rows"))) || ((data.cols === undefined) && (missing0 = "cols"))) || ((data.units === undefined) && (missing0 = "units"))){
validate66.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.cols !== undefined){
let data0 = data.cols;
const _errs1 = errors;
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate67(data0[i0], {instancePath:instancePath+"/cols/" + i0,parentData:data0,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate67.errors : vErrors.concat(validate67.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
else {
validate66.errors = [{instancePath:instancePath+"/cols",schemaPath:"#/properties/cols/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rows !== undefined){
let data2 = data.rows;
const _errs4 = errors;
if(errors === _errs4){
if(Array.isArray(data2)){
var valid2 = true;
const len1 = data2.length;
for(let i1=0; i1<len1; i1++){
const _errs6 = errors;
if(!(validate86(data2[i1], {instancePath:instancePath+"/rows/" + i1,parentData:data2,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate86.errors : vErrors.concat(validate86.errors);
errors = vErrors.length;
}
var valid2 = _errs6 === errors;
if(!valid2){
break;
}
}
}
else {
validate66.errors = [{instancePath:instancePath+"/rows",schemaPath:"#/properties/rows/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.units !== undefined){
const _errs7 = errors;
if(typeof data.units !== "string"){
validate66.errors = [{instancePath:instancePath+"/units",schemaPath:"#/properties/units/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.values !== undefined){
let data5 = data.values;
const _errs9 = errors;
if(errors === _errs9){
if(Array.isArray(data5)){
var valid3 = true;
const len2 = data5.length;
for(let i2=0; i2<len2; i2++){
let data6 = data5[i2];
const _errs11 = errors;
if(errors === _errs11){
if(Array.isArray(data6)){
var valid4 = true;
const len3 = data6.length;
for(let i3=0; i3<len3; i3++){
let data7 = data6[i3];
const _errs13 = errors;
if(errors === _errs13){
if(!((typeof data7 == "number") && (isFinite(data7)))){
validate66.errors = [{instancePath:instancePath+"/values/" + i2+"/" + i3,schemaPath:"#/properties/values/items/items/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid4 = _errs13 === errors;
if(!valid4){
break;
}
}
}
else {
validate66.errors = [{instancePath:instancePath+"/values/" + i2,schemaPath:"#/properties/values/items/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid3 = _errs11 === errors;
if(!valid3){
break;
}
}
}
else {
validate66.errors = [{instancePath:instancePath+"/values",schemaPath:"#/properties/values/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate66.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate66.errors = vErrors;
return errors === 0;
}
validate66.evaluated = {"props":{"cols":true,"rows":true,"units":true,"values":true},"dynamicProps":false,"dynamicItems":false};

const schema40 = {"description":"A solve result as the JSON entry points and adapters emit it: every\n[`SolveResponse`] field at the top level, plus facts about the solved\nproblem that the response does not hold.\n\n`objective_kind` is derived from the problem each time a result is emitted,\nnever stored: it says whether the OPF minimized a declared generator cost\nor only looked for a feasible point. It is present for every OPF\nformulation (`dcopf`, `socwr`, `acopf`) and absent for power flows.","properties":{"branch_constraints":{"description":"Thermal constraint state and marginal relief, when exposed by the formulation.","items":{"$ref":"#/$defs/BranchConstraintResult"},"type":["array","null"]},"branch_susceptance_formula":{"anyOf":[{"$ref":"#/$defs/DcFormula"},{"type":"null"}],"description":"Resolved DC convention; absent for AC results and older saved responses."},"constraints":{"description":"The request's linear constraints, in request order (dcopf). Absent when\nthe request stated none.","items":{"$ref":"#/$defs/ConstraintResult"},"type":["array","null"]},"diagnostics":{"description":"How the solve treated the network's islands. Empty, and omitted, when\nevery island is supplied and referenced as stated.","items":{"$ref":"#/$defs/SolveDiagnostic"},"type":"array"},"dispatch":{"items":{"$ref":"#/$defs/GenDispatch"},"type":["array","null"]},"flows":{"items":{"$ref":"#/$defs/BranchFlow"},"type":["array","null"]},"formulation":{"$ref":"#/$defs/Problem","description":"The formulation that produced this, echoed for the client."},"initialization":{"anyOf":[{"$ref":"#/$defs/AcPfInitialization"},{"type":"null"}],"description":"Successful AC power-flow start. Absent for other formulations and legacy results."},"injections":{"description":"Nodal injections (acpf), MW/MVAr.","items":{"$ref":"#/$defs/BusInjection"},"type":["array","null"]},"iterations":{"anyOf":[{"$ref":"#/$defs/Iterations"},{"type":"null"}]},"limit_rounds":{"description":"The solves a DC OPF ran under the request's `limits` (dcopf with\n`limits` only).","items":{"$ref":"#/$defs/LimitRound"},"type":["array","null"]},"lmp":{"description":"Active nodal price when the solved OPF declares a network generator\ncost objective. Feasibility solves leave this absent.","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"lmp_q":{"description":"Reactive nodal price. Always `None` in this build; the field is retained for\nwire-format stability.","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"objective":{"format":"double","type":["number","null"]},"objective_kind":{"anyOf":[{"$ref":"#/$defs/ObjectiveKind"},{"type":"null"}]},"sensitivities":{"description":"One self-describing matrix per requested cell, in request order. Each carries\nits own row/column element ids and the served-unit label.","items":{"$ref":"#/$defs/SensitivityMatrix"},"type":"array"},"status":{"$ref":"#/$defs/SolveStatus"},"va":{"description":"Voltage angle, radians (every path except socwr, which is W-space).","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"vm":{"description":"Voltage magnitude, per unit (ACPF or square root of SOCWR squared magnitude).","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"w":{"description":"Squared voltage magnitude `w = |V|^2`, per unit squared (socwr). The conic\nrelaxation does not guarantee a globally consistent set of voltage angles.","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"wi":{"items":{"$ref":"#/$defs/BranchScalar"},"type":["array","null"]},"wr":{"description":"Real and imaginary oriented branch voltage products (SOCWR).","items":{"$ref":"#/$defs/BranchScalar"},"type":["array","null"]}},"required":["formulation","status"],"type":"object"};
const schema41 = {"description":"A DC OPF branch's thermal constraint in the final solved program.","properties":{"binding":{"description":"An enforced bound is met to the solver's reporting tolerance.","type":"boolean"},"branch":{"format":"uint","minimum":0,"type":"integer"},"enforced":{"type":"boolean"},"relief_per_mw":{"description":"Objective decrease per additional MW of symmetric rating. Only present\nfor an enforced row with an economic objective.","format":"double","type":["number","null"]}},"required":["branch","enforced","binding"],"type":"object"};

function validate100(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate100.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.branch === undefined) && (missing0 = "branch")) || ((data.enforced === undefined) && (missing0 = "enforced"))) || ((data.binding === undefined) && (missing0 = "binding"))){
validate100.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.binding !== undefined){
const _errs1 = errors;
if(typeof data.binding !== "boolean"){
validate100.errors = [{instancePath:instancePath+"/binding",schemaPath:"#/properties/binding/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.branch !== undefined){
let data1 = data.branch;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate100.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate100.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.enforced !== undefined){
const _errs5 = errors;
if(typeof data.enforced !== "boolean"){
validate100.errors = [{instancePath:instancePath+"/enforced",schemaPath:"#/properties/enforced/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.relief_per_mw !== undefined){
let data3 = data.relief_per_mw;
const _errs7 = errors;
if((!((typeof data3 == "number") && (isFinite(data3)))) && (data3 !== null)){
validate100.errors = [{instancePath:instancePath+"/relief_per_mw",schemaPath:"#/properties/relief_per_mw/type",keyword:"type",params:{type: schema41.properties.relief_per_mw.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate100.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate100.errors = vErrors;
return errors === 0;
}
validate100.evaluated = {"props":{"binding":true,"branch":true,"enforced":true,"relief_per_mw":true},"dynamicProps":false,"dynamicItems":false};

const schema42 = {"description":"The complete DC convention, including taps and phase shifts.","oneOf":[{"const":"series_susceptance","description":"Legacy Tellegen: series susceptance with shifts, ignoring taps.","type":"string"},{"const":"tap_adjusted_reactance","description":"MATPOWER / PowerModels DCMP: ignore resistance, include taps and shifts.","type":"string"},{"const":"reactance_only","description":"Textbook 1/x, ignoring taps and shifts.","type":"string"},{"const":"tap_adjusted_series_susceptance","description":"Ignore conductance: series susceptance divided by tap, with shifts.","type":"string"},{"const":"series_susceptance_no_phase_shift","description":"PowerModels DCP (PGLib DC baseline): ignore taps and shifts.","type":"string"}]};

function validate102(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate102.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("series_susceptance" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "series_susceptance"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("tap_adjusted_reactance" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "tap_adjusted_reactance"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
const _errs5 = errors;
if(typeof data !== "string"){
const err4 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if("reactance_only" !== data){
const err5 = {instancePath,schemaPath:"#/oneOf/2/const",keyword:"const",params:{allowedValue: "reactance_only"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
}
const _errs7 = errors;
if(typeof data !== "string"){
const err6 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if("tap_adjusted_series_susceptance" !== data){
const err7 = {instancePath,schemaPath:"#/oneOf/3/const",keyword:"const",params:{allowedValue: "tap_adjusted_series_susceptance"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
var _valid0 = _errs7 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
}
const _errs9 = errors;
if(typeof data !== "string"){
const err8 = {instancePath,schemaPath:"#/oneOf/4/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
if("series_susceptance_no_phase_shift" !== data){
const err9 = {instancePath,schemaPath:"#/oneOf/4/const",keyword:"const",params:{allowedValue: "series_susceptance_no_phase_shift"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
var _valid0 = _errs9 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 4];
}
else {
if(_valid0){
valid0 = true;
passing0 = 4;
}
}
}
}
}
if(!valid0){
const err10 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
validate102.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate102.errors = vErrors;
return errors === 0;
}
validate102.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema43 = {"description":"The solved state of one [`LinearConstraint`], in request order. `value`,\n`lower`, and `upper` are MW; `shadow_price` is in objective units per MW, the\nsame units as `lmp`.","properties":{"binding":{"description":"`value` sits at a limit, within solver tolerance. Always true for an\nequality.","type":"boolean"},"branches":{"description":"Branches in the resolved row, including flows implied by injection terms.","items":{"format":"uint","minimum":0,"type":"integer"},"type":"array"},"id":{"type":"string"},"lower":{"format":"double","type":["number","null"]},"shadow_price":{"description":"The objective decrease per MW the binding limit is raised:\n`-d objective / d limit`. Positive when the upper limit binds, negative\nwhen the lower limit binds, zero when neither does. A bus's price then\ndiffers from its island's reference bus's by `-shadow_price` times the\nchange in `value` per MW injected at the bus and withdrawn at that reference.\nAbsent when the declared objective gives prices no economic meaning, as\nfor `lmp`.","format":"double","type":["number","null"]},"upper":{"format":"double","type":["number","null"]},"value":{"description":"`sum(coefficient * quantity)` at the solution.","format":"double","type":"number"}},"required":["id","value","binding"],"type":"object"};

function validate104(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate104.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.id === undefined) && (missing0 = "id")) || ((data.value === undefined) && (missing0 = "value"))) || ((data.binding === undefined) && (missing0 = "binding"))){
validate104.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.binding !== undefined){
const _errs1 = errors;
if(typeof data.binding !== "boolean"){
validate104.errors = [{instancePath:instancePath+"/binding",schemaPath:"#/properties/binding/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.branches !== undefined){
let data1 = data.branches;
const _errs3 = errors;
if(errors === _errs3){
if(Array.isArray(data1)){
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
let data2 = data1[i0];
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate104.errors = [{instancePath:instancePath+"/branches/" + i0,schemaPath:"#/properties/branches/items/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate104.errors = [{instancePath:instancePath+"/branches/" + i0,schemaPath:"#/properties/branches/items/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid1 = _errs5 === errors;
if(!valid1){
break;
}
}
}
else {
validate104.errors = [{instancePath:instancePath+"/branches",schemaPath:"#/properties/branches/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.id !== undefined){
const _errs7 = errors;
if(typeof data.id !== "string"){
validate104.errors = [{instancePath:instancePath+"/id",schemaPath:"#/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lower !== undefined){
let data4 = data.lower;
const _errs9 = errors;
if((!((typeof data4 == "number") && (isFinite(data4)))) && (data4 !== null)){
validate104.errors = [{instancePath:instancePath+"/lower",schemaPath:"#/properties/lower/type",keyword:"type",params:{type: schema43.properties.lower.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.shadow_price !== undefined){
let data5 = data.shadow_price;
const _errs11 = errors;
if((!((typeof data5 == "number") && (isFinite(data5)))) && (data5 !== null)){
validate104.errors = [{instancePath:instancePath+"/shadow_price",schemaPath:"#/properties/shadow_price/type",keyword:"type",params:{type: schema43.properties.shadow_price.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.upper !== undefined){
let data6 = data.upper;
const _errs13 = errors;
if((!((typeof data6 == "number") && (isFinite(data6)))) && (data6 !== null)){
validate104.errors = [{instancePath:instancePath+"/upper",schemaPath:"#/properties/upper/type",keyword:"type",params:{type: schema43.properties.upper.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data7 = data.value;
const _errs15 = errors;
if(errors === _errs15){
if(!((typeof data7 == "number") && (isFinite(data7)))){
validate104.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate104.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate104.errors = vErrors;
return errors === 0;
}
validate104.evaluated = {"props":{"binding":true,"branches":true,"id":true,"lower":true,"shadow_price":true,"upper":true,"value":true},"dynamicProps":false,"dynamicItems":false};

const schema44 = {"description":"A finding about how the solve treated the network, such as an island left out\nbecause nothing supplies it. `code` is stable for programs; `message` is for\npeople. `buses` names the original ids of the buses concerned.","properties":{"buses":{"items":{"format":"uint","minimum":0,"type":"integer"},"type":"array"},"code":{"description":"`island_deenergized`: an island with no in-service generator was left\nout of the solve; its buses carry no results and its load is unserved.\nThe message is PowerIO's finding; for an island an HVDC line fed, it\nnames the lines taken out of service with the island and the power\nthey scheduled into it.\n`island_reference_designated`: a supplied island stated no reference bus,\nso the bus of its largest generator became the island's reference.\n`island_extra_reference`: the AC power flow found several reference\nbuses in one island and kept the first as its slack; the others hold\ntheir voltage magnitude as PV (or PQ) buses.\n`BUILD.HVDC.FIXED_INJECTION`: PowerIO's statement of what the in-service\nHVDC lines inject as fixed transfers at their terminal buses.\n`BUILD.HVDC.TERMINAL_INACTIVE`: an in-service HVDC line has a terminal bus\noutside the solve, so neither end injects; `buses` are its two terminals.\n`settings_ignored`: a reopened Study could not apply some or all of the\ncalculation settings its saved module recorded, such as an option this\nengine does not know; engine defaults stand in for what was dropped.","type":"string"},"message":{"type":"string"}},"required":["code","message","buses"],"type":"object"};

function validate106(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate106.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.code === undefined) && (missing0 = "code")) || ((data.message === undefined) && (missing0 = "message"))) || ((data.buses === undefined) && (missing0 = "buses"))){
validate106.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.buses !== undefined){
let data0 = data.buses;
const _errs1 = errors;
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
let data1 = data0[i0];
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate106.errors = [{instancePath:instancePath+"/buses/" + i0,schemaPath:"#/properties/buses/items/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate106.errors = [{instancePath:instancePath+"/buses/" + i0,schemaPath:"#/properties/buses/items/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
else {
validate106.errors = [{instancePath:instancePath+"/buses",schemaPath:"#/properties/buses/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.code !== undefined){
const _errs5 = errors;
if(typeof data.code !== "string"){
validate106.errors = [{instancePath:instancePath+"/code",schemaPath:"#/properties/code/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.message !== undefined){
const _errs7 = errors;
if(typeof data.message !== "string"){
validate106.errors = [{instancePath:instancePath+"/message",schemaPath:"#/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate106.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate106.errors = vErrors;
return errors === 0;
}
validate106.evaluated = {"props":{"buses":true,"code":true,"message":true},"dynamicProps":false,"dynamicItems":false};

const schema45 = {"description":"Generator dispatch, keyed by original generator id. `qg` is `None` on the DC paths.","properties":{"bus":{"description":"Original bus identity, allowing exact aggregation of co-located dispatch.","format":"uint","minimum":0,"type":["integer","null"]},"gen":{"format":"uint","minimum":0,"type":"integer"},"pg":{"format":"double","type":"number"},"qg":{"format":"double","type":["number","null"]}},"required":["gen","pg"],"type":"object"};

function validate108(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate108.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.gen === undefined) && (missing0 = "gen")) || ((data.pg === undefined) && (missing0 = "pg"))){
validate108.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if((!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))) && (data0 !== null)){
validate108.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: schema45.properties.bus.type},message:"must be integer,null"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate108.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.gen !== undefined){
let data1 = data.gen;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate108.errors = [{instancePath:instancePath+"/gen",schemaPath:"#/properties/gen/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate108.errors = [{instancePath:instancePath+"/gen",schemaPath:"#/properties/gen/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.pg !== undefined){
let data2 = data.pg;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate108.errors = [{instancePath:instancePath+"/pg",schemaPath:"#/properties/pg/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.qg !== undefined){
let data3 = data.qg;
const _errs7 = errors;
if((!((typeof data3 == "number") && (isFinite(data3)))) && (data3 !== null)){
validate108.errors = [{instancePath:instancePath+"/qg",schemaPath:"#/properties/qg/type",keyword:"type",params:{type: schema45.properties.qg.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate108.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate108.errors = vErrors;
return errors === 0;
}
validate108.evaluated = {"props":{"bus":true,"gen":true,"pg":true,"qg":true},"dynamicProps":false,"dynamicItems":false};

const schema46 = {"description":"Branch flows, keyed by original branch id (plus the row uid when carried, as in\n[`BusScalar`]). `pf` (from-end active, MW) and `loading` (|S|/limit,\ndimensionless) are present on every formulation that has flows; the reactive\nand to-end legs are `None` on the DC paths.","properties":{"branch":{"format":"uint","minimum":0,"type":"integer"},"loading":{"format":"double","type":"number"},"loading_info":{"anyOf":[{"$ref":"#/$defs/BranchLoading"},{"type":"null"}]},"pf":{"format":"double","type":"number"},"pt":{"format":"double","type":["number","null"]},"qf":{"format":"double","type":["number","null"]},"qt":{"format":"double","type":["number","null"]},"uid":{"type":["string","null"]}},"required":["branch","pf","loading"],"type":"object"};
const schema47 = {"description":"Interpretation of `loading`, independent of economic binding or enforcement.\nAn absent rating means no known equipment limit, even if a numerical solver\nfallback produced a finite utilization. Older responses omit this object.","properties":{"basis":{"$ref":"#/$defs/LoadingBasis"},"limiting_end":{"anyOf":[{"$ref":"#/$defs/LoadingEnd"},{"type":"null"}]},"rating":{"format":"double","type":["number","null"]},"rating_source":{"anyOf":[{"$ref":"#/$defs/RatingSource"},{"type":"null"}]}},"required":["basis"],"type":"object"};
const schema48 = {"description":"Basis of the dimensionless branch utilization, with the rating in MW or MVA.","enum":["mw","mva"],"type":"string"};

function validate112(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate112.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate112.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "mw") || (data === "mva"))){
validate112.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema48.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate112.errors = vErrors;
return errors === 0;
}
validate112.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema49 = {"enum":["from","to","both"],"type":"string"};

function validate114(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate114.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate114.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!(((data === "from") || (data === "to")) || (data === "both"))){
validate114.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema49.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate114.errors = vErrors;
return errors === 0;
}
validate114.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema50 = {"enum":["stated","synthesized"],"type":"string"};

function validate116(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate116.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate116.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "stated") || (data === "synthesized"))){
validate116.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema50.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate116.errors = vErrors;
return errors === 0;
}
validate116.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate111(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate111.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((data.basis === undefined) && (missing0 = "basis")){
validate111.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.basis !== undefined){
const _errs1 = errors;
if(!(validate112(data.basis, {instancePath:instancePath+"/basis",parentData:data,parentDataProperty:"basis",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate112.errors : vErrors.concat(validate112.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.limiting_end !== undefined){
let data1 = data.limiting_end;
const _errs2 = errors;
const _errs3 = errors;
let valid1 = false;
const _errs4 = errors;
if(!(validate114(data1, {instancePath:instancePath+"/limiting_end",parentData:data,parentDataProperty:"limiting_end",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate114.errors : vErrors.concat(validate114.errors);
errors = vErrors.length;
}
var _valid0 = _errs4 === errors;
valid1 = valid1 || _valid0;
const _errs5 = errors;
if(data1 !== null){
const err0 = {instancePath:instancePath+"/limiting_end",schemaPath:"#/properties/limiting_end/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs5 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/limiting_end",schemaPath:"#/properties/limiting_end/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate111.errors = vErrors;
return false;
}
else {
errors = _errs3;
if(vErrors !== null){
if(_errs3){
vErrors.length = _errs3;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rating !== undefined){
let data2 = data.rating;
const _errs7 = errors;
if((!((typeof data2 == "number") && (isFinite(data2)))) && (data2 !== null)){
validate111.errors = [{instancePath:instancePath+"/rating",schemaPath:"#/properties/rating/type",keyword:"type",params:{type: schema47.properties.rating.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rating_source !== undefined){
let data3 = data.rating_source;
const _errs9 = errors;
const _errs10 = errors;
let valid2 = false;
const _errs11 = errors;
if(!(validate116(data3, {instancePath:instancePath+"/rating_source",parentData:data,parentDataProperty:"rating_source",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate116.errors : vErrors.concat(validate116.errors);
errors = vErrors.length;
}
var _valid1 = _errs11 === errors;
valid2 = valid2 || _valid1;
const _errs12 = errors;
if(data3 !== null){
const err2 = {instancePath:instancePath+"/rating_source",schemaPath:"#/properties/rating_source/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs12 === errors;
valid2 = valid2 || _valid1;
if(!valid2){
const err3 = {instancePath:instancePath+"/rating_source",schemaPath:"#/properties/rating_source/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate111.errors = vErrors;
return false;
}
else {
errors = _errs10;
if(vErrors !== null){
if(_errs10){
vErrors.length = _errs10;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate111.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate111.errors = vErrors;
return errors === 0;
}
validate111.evaluated = {"props":{"basis":true,"limiting_end":true,"rating":true,"rating_source":true},"dynamicProps":false,"dynamicItems":false};


function validate110(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate110.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.branch === undefined) && (missing0 = "branch")) || ((data.pf === undefined) && (missing0 = "pf"))) || ((data.loading === undefined) && (missing0 = "loading"))){
validate110.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch !== undefined){
let data0 = data.branch;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate110.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate110.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.loading !== undefined){
let data1 = data.loading;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate110.errors = [{instancePath:instancePath+"/loading",schemaPath:"#/properties/loading/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.loading_info !== undefined){
let data2 = data.loading_info;
const _errs5 = errors;
const _errs6 = errors;
let valid1 = false;
const _errs7 = errors;
if(!(validate111(data2, {instancePath:instancePath+"/loading_info",parentData:data,parentDataProperty:"loading_info",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate111.errors : vErrors.concat(validate111.errors);
errors = vErrors.length;
}
var _valid0 = _errs7 === errors;
valid1 = valid1 || _valid0;
if(_valid0){
var props0 = {};
props0.basis = true;
props0.limiting_end = true;
props0.rating = true;
props0.rating_source = true;
}
const _errs8 = errors;
if(data2 !== null){
const err0 = {instancePath:instancePath+"/loading_info",schemaPath:"#/properties/loading_info/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs8 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/loading_info",schemaPath:"#/properties/loading_info/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate110.errors = vErrors;
return false;
}
else {
errors = _errs6;
if(vErrors !== null){
if(_errs6){
vErrors.length = _errs6;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.pf !== undefined){
let data3 = data.pf;
const _errs10 = errors;
if(errors === _errs10){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate110.errors = [{instancePath:instancePath+"/pf",schemaPath:"#/properties/pf/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.pt !== undefined){
let data4 = data.pt;
const _errs12 = errors;
if((!((typeof data4 == "number") && (isFinite(data4)))) && (data4 !== null)){
validate110.errors = [{instancePath:instancePath+"/pt",schemaPath:"#/properties/pt/type",keyword:"type",params:{type: schema46.properties.pt.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.qf !== undefined){
let data5 = data.qf;
const _errs14 = errors;
if((!((typeof data5 == "number") && (isFinite(data5)))) && (data5 !== null)){
validate110.errors = [{instancePath:instancePath+"/qf",schemaPath:"#/properties/qf/type",keyword:"type",params:{type: schema46.properties.qf.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs14 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.qt !== undefined){
let data6 = data.qt;
const _errs16 = errors;
if((!((typeof data6 == "number") && (isFinite(data6)))) && (data6 !== null)){
validate110.errors = [{instancePath:instancePath+"/qt",schemaPath:"#/properties/qt/type",keyword:"type",params:{type: schema46.properties.qt.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs16 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.uid !== undefined){
let data7 = data.uid;
const _errs18 = errors;
if((typeof data7 !== "string") && (data7 !== null)){
validate110.errors = [{instancePath:instancePath+"/uid",schemaPath:"#/properties/uid/type",keyword:"type",params:{type: schema46.properties.uid.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
else {
validate110.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate110.errors = vErrors;
return errors === 0;
}
validate110.evaluated = {"props":{"branch":true,"loading":true,"loading_info":true,"pf":true,"pt":true,"qf":true,"qt":true,"uid":true},"dynamicProps":false,"dynamicItems":false};

const schema51 = {"description":"Which problem to solve. The convex/power flow solve paths, as the lowercase JSON\ntags `\"dcpf\"`/`\"dcopf\"`/`\"acpf\"`/`\"socwr\"`. A plain (not internally tagged) enum\nso a request that omits it defaults to [`DcOpf`](Problem::DcOpf), and `{}` is a\nvalid base-case DC OPF request.\n\nThe `\"acopf\"` tag enables full nonlinear AC OPF only when the opt-in `acopf`\nfeature is built. Shipping adapters do not forward that feature.","oneOf":[{"const":"dcpf","description":"DC power flow: angles and flows at the fixed generator setpoints. No prices,\nno dispatch, no sensitivity.","type":"string"},{"const":"dcopf","description":"DC OPF: the LMP / dispatch / flow workhorse. Differentiable via the DC KKT.","type":"string"},{"const":"acpf","description":"AC polar Newton power flow. Voltages and nodal injections. Differentiable\nvia the AC Newton system.","type":"string"},{"const":"socwr","description":"SOCWR (Jabr) conic relaxation of AC OPF. Differentiable via the conic KKT.","type":"string"},{"const":"acopf","description":"Full nonlinear AC OPF through the opt-in POUNCE backend.","type":"string"}]};

function validate120(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate120.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("dcpf" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "dcpf"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("dcopf" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "dcopf"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
const _errs5 = errors;
if(typeof data !== "string"){
const err4 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if("acpf" !== data){
const err5 = {instancePath,schemaPath:"#/oneOf/2/const",keyword:"const",params:{allowedValue: "acpf"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
}
const _errs7 = errors;
if(typeof data !== "string"){
const err6 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if("socwr" !== data){
const err7 = {instancePath,schemaPath:"#/oneOf/3/const",keyword:"const",params:{allowedValue: "socwr"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
var _valid0 = _errs7 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
}
const _errs9 = errors;
if(typeof data !== "string"){
const err8 = {instancePath,schemaPath:"#/oneOf/4/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
if("acopf" !== data){
const err9 = {instancePath,schemaPath:"#/oneOf/4/const",keyword:"const",params:{allowedValue: "acopf"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
var _valid0 = _errs9 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 4];
}
else {
if(_valid0){
valid0 = true;
passing0 = 4;
}
}
}
}
}
if(!valid0){
const err10 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
validate120.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate120.errors = vErrors;
return errors === 0;
}
validate120.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema52 = {"description":"Successful AC initialization, including any fallback from the requested start.","properties":{"attempts":{"description":"Starts attempted, including the successful one (one-based).","format":"uint","minimum":0,"type":"integer"},"requested":{"$ref":"#/$defs/PowerFlowStart"},"restart":{"description":"Deterministic perturbation number (one-based), only for `perturbed`.","format":"uint","minimum":0,"type":["integer","null"]},"used":{"$ref":"#/$defs/AcPfStartStrategy"}},"required":["requested","used","attempts"],"type":"object"};
const schema53 = {"description":"Where the AC power flow's Newton iteration starts, as the lowercase JSON tags\n`\"flat\"` and `\"case\"`. Only `acpf` iterates from a start; every other\nformulation refuses `\"case\"`.","oneOf":[{"const":"flat","description":"Voltage setpoint magnitudes and every angle at its island's reference angle.","type":"string"},{"const":"case","description":"The case's stored bus voltages: each bus's magnitude and its angle\nrelative to its island's reference bus (star buses of three-winding transformers\nwhere the source carries them), or a typed instance's initial point. A\nstored state that does not converge falls back to the flat start.","type":"string"}]};

function validate123(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate123.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("flat" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "flat"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("case" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "case"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
}
if(!valid0){
const err4 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
validate123.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate123.errors = vErrors;
return errors === 0;
}
validate123.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema54 = {"description":"Initialization that produced a converged AC power flow.","enum":["case","flat","perturbed"],"type":"string"};

function validate125(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate125.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate125.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!(((data === "case") || (data === "flat")) || (data === "perturbed"))){
validate125.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema54.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate125.errors = vErrors;
return errors === 0;
}
validate125.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate122(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate122.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.requested === undefined) && (missing0 = "requested")) || ((data.used === undefined) && (missing0 = "used"))) || ((data.attempts === undefined) && (missing0 = "attempts"))){
validate122.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.attempts !== undefined){
let data0 = data.attempts;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate122.errors = [{instancePath:instancePath+"/attempts",schemaPath:"#/properties/attempts/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate122.errors = [{instancePath:instancePath+"/attempts",schemaPath:"#/properties/attempts/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.requested !== undefined){
const _errs3 = errors;
if(!(validate123(data.requested, {instancePath:instancePath+"/requested",parentData:data,parentDataProperty:"requested",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate123.errors : vErrors.concat(validate123.errors);
errors = vErrors.length;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.restart !== undefined){
let data2 = data.restart;
const _errs4 = errors;
if((!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))) && (data2 !== null)){
validate122.errors = [{instancePath:instancePath+"/restart",schemaPath:"#/properties/restart/type",keyword:"type",params:{type: schema52.properties.restart.type},message:"must be integer,null"}];
return false;
}
if(errors === _errs4){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate122.errors = [{instancePath:instancePath+"/restart",schemaPath:"#/properties/restart/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.used !== undefined){
const _errs6 = errors;
if(!(validate125(data.used, {instancePath:instancePath+"/used",parentData:data,parentDataProperty:"used",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate125.errors : vErrors.concat(validate125.errors);
errors = vErrors.length;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate122.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate122.errors = vErrors;
return errors === 0;
}
validate122.evaluated = {"props":{"attempts":true,"requested":true,"restart":true,"used":true},"dynamicProps":false,"dynamicItems":false};

const schema55 = {"description":"A nodal net injection (MW / MVAr), keyed by original bus id (plus the row uid\nwhen carried, as in [`BusScalar`]). Injections describe the net nodal power balance.","properties":{"bus":{"format":"uint","minimum":0,"type":"integer"},"p":{"format":"double","type":"number"},"q":{"format":"double","type":"number"},"uid":{"type":["string","null"]}},"required":["bus","p","q"],"type":"object"};

function validate128(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate128.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.bus === undefined) && (missing0 = "bus")) || ((data.p === undefined) && (missing0 = "p"))) || ((data.q === undefined) && (missing0 = "q"))){
validate128.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate128.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate128.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.p !== undefined){
let data1 = data.p;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate128.errors = [{instancePath:instancePath+"/p",schemaPath:"#/properties/p/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.q !== undefined){
let data2 = data.q;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate128.errors = [{instancePath:instancePath+"/q",schemaPath:"#/properties/q/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.uid !== undefined){
let data3 = data.uid;
const _errs7 = errors;
if((typeof data3 !== "string") && (data3 !== null)){
validate128.errors = [{instancePath:instancePath+"/uid",schemaPath:"#/properties/uid/type",keyword:"type",params:{type: schema55.properties.uid.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate128.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate128.errors = vErrors;
return errors === 0;
}
validate128.evaluated = {"props":{"bus":true,"p":true,"q":true,"uid":true},"dynamicProps":false,"dynamicItems":false};

const schema56 = {"description":"One solve of a DC OPF run under [`LimitOptions`]: the program it solved and\nhow the solution met the enforceable thermal limits. A run without lazy\nenforcement has one round.","properties":{"added":{"description":"Limits added for the next round: the unenforced ones the flows violate\nor meet, and those loaded at or above `near_binding`.","format":"uint","minimum":0,"type":"integer"},"enforced":{"description":"Thermal limits the round's program enforced.","format":"uint","minimum":0,"type":"integer"},"ipm_iterations":{"description":"Interior-point iterations of the round's solve.","format":"uint","minimum":0,"type":"integer"},"max_loading":{"description":"The largest loading over the enforceable limits, as a fraction.","format":"double","type":"number"},"rows":{"format":"uint","minimum":0,"type":"integer"},"variables":{"description":"Variables and constraint rows of the round's program.","format":"uint","minimum":0,"type":"integer"},"violated":{"description":"Enforceable limits the round's solution violated; zero on the last round.","format":"uint","minimum":0,"type":"integer"}},"required":["enforced","violated","added","max_loading","variables","rows","ipm_iterations"],"type":"object"};

function validate131(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate131.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.enforced === undefined) && (missing0 = "enforced")) || ((data.violated === undefined) && (missing0 = "violated"))) || ((data.added === undefined) && (missing0 = "added"))) || ((data.max_loading === undefined) && (missing0 = "max_loading"))) || ((data.variables === undefined) && (missing0 = "variables"))) || ((data.rows === undefined) && (missing0 = "rows"))) || ((data.ipm_iterations === undefined) && (missing0 = "ipm_iterations"))){
validate131.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.added !== undefined){
let data0 = data.added;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate131.errors = [{instancePath:instancePath+"/added",schemaPath:"#/properties/added/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate131.errors = [{instancePath:instancePath+"/added",schemaPath:"#/properties/added/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.enforced !== undefined){
let data1 = data.enforced;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate131.errors = [{instancePath:instancePath+"/enforced",schemaPath:"#/properties/enforced/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate131.errors = [{instancePath:instancePath+"/enforced",schemaPath:"#/properties/enforced/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.ipm_iterations !== undefined){
let data2 = data.ipm_iterations;
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate131.errors = [{instancePath:instancePath+"/ipm_iterations",schemaPath:"#/properties/ipm_iterations/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate131.errors = [{instancePath:instancePath+"/ipm_iterations",schemaPath:"#/properties/ipm_iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.max_loading !== undefined){
let data3 = data.max_loading;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate131.errors = [{instancePath:instancePath+"/max_loading",schemaPath:"#/properties/max_loading/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rows !== undefined){
let data4 = data.rows;
const _errs9 = errors;
if(!(((typeof data4 == "number") && (!(data4 % 1) && !isNaN(data4))) && (isFinite(data4)))){
validate131.errors = [{instancePath:instancePath+"/rows",schemaPath:"#/properties/rows/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs9){
if((typeof data4 == "number") && (isFinite(data4))){
if(data4 < 0 || isNaN(data4)){
validate131.errors = [{instancePath:instancePath+"/rows",schemaPath:"#/properties/rows/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.variables !== undefined){
let data5 = data.variables;
const _errs11 = errors;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
validate131.errors = [{instancePath:instancePath+"/variables",schemaPath:"#/properties/variables/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs11){
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
validate131.errors = [{instancePath:instancePath+"/variables",schemaPath:"#/properties/variables/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.violated !== undefined){
let data6 = data.violated;
const _errs13 = errors;
if(!(((typeof data6 == "number") && (!(data6 % 1) && !isNaN(data6))) && (isFinite(data6)))){
validate131.errors = [{instancePath:instancePath+"/violated",schemaPath:"#/properties/violated/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs13){
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 < 0 || isNaN(data6)){
validate131.errors = [{instancePath:instancePath+"/violated",schemaPath:"#/properties/violated/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate131.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate131.errors = vErrors;
return errors === 0;
}
validate131.evaluated = {"props":{"added":true,"enforced":true,"ipm_iterations":true,"max_loading":true,"rows":true,"variables":true,"violated":true},"dynamicProps":false,"dynamicItems":false};

const schema57 = {"description":"A scalar keyed by original bus id (LMP, voltage, angle, squared magnitude).\n`uid` is the bus's powerio row uid when the solved network carried one, so an\noverlay can re-key on stable identity instead of the positional id.","properties":{"bus":{"format":"uint","minimum":0,"type":"integer"},"uid":{"type":["string","null"]},"value":{"format":"double","type":"number"}},"required":["bus","value"],"type":"object"};

function validate133(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate133.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.bus === undefined) && (missing0 = "bus")) || ((data.value === undefined) && (missing0 = "value"))){
validate133.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate133.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate133.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.uid !== undefined){
let data1 = data.uid;
const _errs3 = errors;
if((typeof data1 !== "string") && (data1 !== null)){
validate133.errors = [{instancePath:instancePath+"/uid",schemaPath:"#/properties/uid/type",keyword:"type",params:{type: schema57.properties.uid.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data2 = data.value;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate133.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate133.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate133.errors = vErrors;
return errors === 0;
}
validate133.evaluated = {"props":{"bus":true,"uid":true,"value":true},"dynamicProps":false,"dynamicItems":false};

const schema58 = {"description":"What the solved OPF minimized, as PowerIO declared it for the case. A case\nwith no generator cost data declares a feasibility problem: the reported\n`objective` is then zero by construction and prices and limit multipliers\nare left out, because any feasible point is optimal and they are not unique.","oneOf":[{"const":"cost","description":"Total generator cost, in the case's cost units.","type":"string"},{"const":"feasibility","description":"No cost was declared; the solve found a feasible operating point.","type":"string"}]};

function validate136(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate136.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("cost" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "cost"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("feasibility" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "feasibility"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
}
if(!valid0){
const err4 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
validate136.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate136.errors = vErrors;
return errors === 0;
}
validate136.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema59 = {"description":"A solve outcome that succeeded. A failed solve is the `Err` arm of a solve entry.","oneOf":[{"const":"optimal","description":"An OPF reached optimality.","type":"string"},{"const":"feasible","description":"A power flow or local AC OPF solve reached a validated feasible point.","type":"string"}]};

function validate139(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate139.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("optimal" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "optimal"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("feasible" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "feasible"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
}
if(!valid0){
const err4 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
validate139.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate139.errors = vErrors;
return errors === 0;
}
validate139.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema60 = {"description":"A branch-indexed voltage product, in squared per unit.","properties":{"branch":{"format":"uint","minimum":0,"type":"integer"},"value":{"format":"double","type":"number"}},"required":["branch","value"],"type":"object"};

function validate144(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate144.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.branch === undefined) && (missing0 = "branch")) || ((data.value === undefined) && (missing0 = "value"))){
validate144.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch !== undefined){
let data0 = data.branch;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate144.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate144.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data1 = data.value;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate144.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate144.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate144.errors = vErrors;
return errors === 0;
}
validate144.evaluated = {"props":{"branch":true,"value":true},"dynamicProps":false,"dynamicItems":false};


function validate99(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate99.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.formulation === undefined) && (missing0 = "formulation")) || ((data.status === undefined) && (missing0 = "status"))){
validate99.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch_constraints !== undefined){
let data0 = data.branch_constraints;
const _errs1 = errors;
if((!(Array.isArray(data0))) && (data0 !== null)){
validate99.errors = [{instancePath:instancePath+"/branch_constraints",schemaPath:"#/properties/branch_constraints/type",keyword:"type",params:{type: schema40.properties.branch_constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate100(data0[i0], {instancePath:instancePath+"/branch_constraints/" + i0,parentData:data0,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate100.errors : vErrors.concat(validate100.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.branch_susceptance_formula !== undefined){
let data2 = data.branch_susceptance_formula;
const _errs4 = errors;
const _errs5 = errors;
let valid2 = false;
const _errs6 = errors;
if(!(validate102(data2, {instancePath:instancePath+"/branch_susceptance_formula",parentData:data,parentDataProperty:"branch_susceptance_formula",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate102.errors : vErrors.concat(validate102.errors);
errors = vErrors.length;
}
var _valid0 = _errs6 === errors;
valid2 = valid2 || _valid0;
const _errs7 = errors;
if(data2 !== null){
const err0 = {instancePath:instancePath+"/branch_susceptance_formula",schemaPath:"#/properties/branch_susceptance_formula/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs7 === errors;
valid2 = valid2 || _valid0;
if(!valid2){
const err1 = {instancePath:instancePath+"/branch_susceptance_formula",schemaPath:"#/properties/branch_susceptance_formula/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate99.errors = vErrors;
return false;
}
else {
errors = _errs5;
if(vErrors !== null){
if(_errs5){
vErrors.length = _errs5;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.constraints !== undefined){
let data3 = data.constraints;
const _errs9 = errors;
if((!(Array.isArray(data3))) && (data3 !== null)){
validate99.errors = [{instancePath:instancePath+"/constraints",schemaPath:"#/properties/constraints/type",keyword:"type",params:{type: schema40.properties.constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs9){
if(Array.isArray(data3)){
var valid3 = true;
const len1 = data3.length;
for(let i1=0; i1<len1; i1++){
const _errs11 = errors;
if(!(validate104(data3[i1], {instancePath:instancePath+"/constraints/" + i1,parentData:data3,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate104.errors : vErrors.concat(validate104.errors);
errors = vErrors.length;
}
var valid3 = _errs11 === errors;
if(!valid3){
break;
}
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.diagnostics !== undefined){
let data5 = data.diagnostics;
const _errs12 = errors;
if(errors === _errs12){
if(Array.isArray(data5)){
var valid4 = true;
const len2 = data5.length;
for(let i2=0; i2<len2; i2++){
const _errs14 = errors;
if(!(validate106(data5[i2], {instancePath:instancePath+"/diagnostics/" + i2,parentData:data5,parentDataProperty:i2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate106.errors : vErrors.concat(validate106.errors);
errors = vErrors.length;
}
var valid4 = _errs14 === errors;
if(!valid4){
break;
}
}
}
else {
validate99.errors = [{instancePath:instancePath+"/diagnostics",schemaPath:"#/properties/diagnostics/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.dispatch !== undefined){
let data7 = data.dispatch;
const _errs15 = errors;
if((!(Array.isArray(data7))) && (data7 !== null)){
validate99.errors = [{instancePath:instancePath+"/dispatch",schemaPath:"#/properties/dispatch/type",keyword:"type",params:{type: schema40.properties.dispatch.type},message:"must be array,null"}];
return false;
}
if(errors === _errs15){
if(Array.isArray(data7)){
var valid5 = true;
const len3 = data7.length;
for(let i3=0; i3<len3; i3++){
const _errs17 = errors;
if(!(validate108(data7[i3], {instancePath:instancePath+"/dispatch/" + i3,parentData:data7,parentDataProperty:i3,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate108.errors : vErrors.concat(validate108.errors);
errors = vErrors.length;
}
var valid5 = _errs17 === errors;
if(!valid5){
break;
}
}
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.flows !== undefined){
let data9 = data.flows;
const _errs18 = errors;
if((!(Array.isArray(data9))) && (data9 !== null)){
validate99.errors = [{instancePath:instancePath+"/flows",schemaPath:"#/properties/flows/type",keyword:"type",params:{type: schema40.properties.flows.type},message:"must be array,null"}];
return false;
}
if(errors === _errs18){
if(Array.isArray(data9)){
var valid6 = true;
const len4 = data9.length;
for(let i4=0; i4<len4; i4++){
const _errs20 = errors;
if(!(validate110(data9[i4], {instancePath:instancePath+"/flows/" + i4,parentData:data9,parentDataProperty:i4,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate110.errors : vErrors.concat(validate110.errors);
errors = vErrors.length;
}
var valid6 = _errs20 === errors;
if(!valid6){
break;
}
}
}
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.formulation !== undefined){
const _errs21 = errors;
if(!(validate120(data.formulation, {instancePath:instancePath+"/formulation",parentData:data,parentDataProperty:"formulation",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate120.errors : vErrors.concat(validate120.errors);
errors = vErrors.length;
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.initialization !== undefined){
let data12 = data.initialization;
const _errs22 = errors;
const _errs23 = errors;
let valid7 = false;
const _errs24 = errors;
if(!(validate122(data12, {instancePath:instancePath+"/initialization",parentData:data,parentDataProperty:"initialization",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate122.errors : vErrors.concat(validate122.errors);
errors = vErrors.length;
}
var _valid1 = _errs24 === errors;
valid7 = valid7 || _valid1;
if(_valid1){
var props0 = {};
props0.attempts = true;
props0.requested = true;
props0.restart = true;
props0.used = true;
}
const _errs25 = errors;
if(data12 !== null){
const err2 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs25 === errors;
valid7 = valid7 || _valid1;
if(!valid7){
const err3 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate99.errors = vErrors;
return false;
}
else {
errors = _errs23;
if(vErrors !== null){
if(_errs23){
vErrors.length = _errs23;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs22 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.injections !== undefined){
let data13 = data.injections;
const _errs27 = errors;
if((!(Array.isArray(data13))) && (data13 !== null)){
validate99.errors = [{instancePath:instancePath+"/injections",schemaPath:"#/properties/injections/type",keyword:"type",params:{type: schema40.properties.injections.type},message:"must be array,null"}];
return false;
}
if(errors === _errs27){
if(Array.isArray(data13)){
var valid8 = true;
const len5 = data13.length;
for(let i5=0; i5<len5; i5++){
const _errs29 = errors;
if(!(validate128(data13[i5], {instancePath:instancePath+"/injections/" + i5,parentData:data13,parentDataProperty:i5,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate128.errors : vErrors.concat(validate128.errors);
errors = vErrors.length;
}
var valid8 = _errs29 === errors;
if(!valid8){
break;
}
}
}
}
var valid0 = _errs27 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iterations !== undefined){
let data15 = data.iterations;
const _errs30 = errors;
const _errs31 = errors;
let valid9 = false;
const _errs32 = errors;
if(!(validate60(data15, {instancePath:instancePath+"/iterations",parentData:data,parentDataProperty:"iterations",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate60.errors : vErrors.concat(validate60.errors);
errors = vErrors.length;
}
var _valid2 = _errs32 === errors;
valid9 = valid9 || _valid2;
const _errs33 = errors;
if(data15 !== null){
const err4 = {instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
var _valid2 = _errs33 === errors;
valid9 = valid9 || _valid2;
if(!valid9){
const err5 = {instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
validate99.errors = vErrors;
return false;
}
else {
errors = _errs31;
if(vErrors !== null){
if(_errs31){
vErrors.length = _errs31;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs30 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.limit_rounds !== undefined){
let data16 = data.limit_rounds;
const _errs35 = errors;
if((!(Array.isArray(data16))) && (data16 !== null)){
validate99.errors = [{instancePath:instancePath+"/limit_rounds",schemaPath:"#/properties/limit_rounds/type",keyword:"type",params:{type: schema40.properties.limit_rounds.type},message:"must be array,null"}];
return false;
}
if(errors === _errs35){
if(Array.isArray(data16)){
var valid10 = true;
const len6 = data16.length;
for(let i6=0; i6<len6; i6++){
const _errs37 = errors;
if(!(validate131(data16[i6], {instancePath:instancePath+"/limit_rounds/" + i6,parentData:data16,parentDataProperty:i6,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate131.errors : vErrors.concat(validate131.errors);
errors = vErrors.length;
}
var valid10 = _errs37 === errors;
if(!valid10){
break;
}
}
}
}
var valid0 = _errs35 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lmp !== undefined){
let data18 = data.lmp;
const _errs38 = errors;
if((!(Array.isArray(data18))) && (data18 !== null)){
validate99.errors = [{instancePath:instancePath+"/lmp",schemaPath:"#/properties/lmp/type",keyword:"type",params:{type: schema40.properties.lmp.type},message:"must be array,null"}];
return false;
}
if(errors === _errs38){
if(Array.isArray(data18)){
var valid11 = true;
const len7 = data18.length;
for(let i7=0; i7<len7; i7++){
const _errs40 = errors;
if(!(validate133(data18[i7], {instancePath:instancePath+"/lmp/" + i7,parentData:data18,parentDataProperty:i7,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid11 = _errs40 === errors;
if(!valid11){
break;
}
}
}
}
var valid0 = _errs38 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lmp_q !== undefined){
let data20 = data.lmp_q;
const _errs41 = errors;
if((!(Array.isArray(data20))) && (data20 !== null)){
validate99.errors = [{instancePath:instancePath+"/lmp_q",schemaPath:"#/properties/lmp_q/type",keyword:"type",params:{type: schema40.properties.lmp_q.type},message:"must be array,null"}];
return false;
}
if(errors === _errs41){
if(Array.isArray(data20)){
var valid12 = true;
const len8 = data20.length;
for(let i8=0; i8<len8; i8++){
const _errs43 = errors;
if(!(validate133(data20[i8], {instancePath:instancePath+"/lmp_q/" + i8,parentData:data20,parentDataProperty:i8,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid12 = _errs43 === errors;
if(!valid12){
break;
}
}
}
}
var valid0 = _errs41 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
let data22 = data.objective;
const _errs44 = errors;
if((!((typeof data22 == "number") && (isFinite(data22)))) && (data22 !== null)){
validate99.errors = [{instancePath:instancePath+"/objective",schemaPath:"#/properties/objective/type",keyword:"type",params:{type: schema40.properties.objective.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs44 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective_kind !== undefined){
let data23 = data.objective_kind;
const _errs46 = errors;
const _errs47 = errors;
let valid13 = false;
const _errs48 = errors;
if(!(validate136(data23, {instancePath:instancePath+"/objective_kind",parentData:data,parentDataProperty:"objective_kind",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate136.errors : vErrors.concat(validate136.errors);
errors = vErrors.length;
}
var _valid3 = _errs48 === errors;
valid13 = valid13 || _valid3;
const _errs49 = errors;
if(data23 !== null){
const err6 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
var _valid3 = _errs49 === errors;
valid13 = valid13 || _valid3;
if(!valid13){
const err7 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
validate99.errors = vErrors;
return false;
}
else {
errors = _errs47;
if(vErrors !== null){
if(_errs47){
vErrors.length = _errs47;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs46 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.sensitivities !== undefined){
let data24 = data.sensitivities;
const _errs51 = errors;
if(errors === _errs51){
if(Array.isArray(data24)){
var valid14 = true;
const len9 = data24.length;
for(let i9=0; i9<len9; i9++){
const _errs53 = errors;
if(!(validate66(data24[i9], {instancePath:instancePath+"/sensitivities/" + i9,parentData:data24,parentDataProperty:i9,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate66.errors : vErrors.concat(validate66.errors);
errors = vErrors.length;
}
var valid14 = _errs53 === errors;
if(!valid14){
break;
}
}
}
else {
validate99.errors = [{instancePath:instancePath+"/sensitivities",schemaPath:"#/properties/sensitivities/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs51 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.status !== undefined){
const _errs54 = errors;
if(!(validate139(data.status, {instancePath:instancePath+"/status",parentData:data,parentDataProperty:"status",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate139.errors : vErrors.concat(validate139.errors);
errors = vErrors.length;
}
var valid0 = _errs54 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.va !== undefined){
let data27 = data.va;
const _errs55 = errors;
if((!(Array.isArray(data27))) && (data27 !== null)){
validate99.errors = [{instancePath:instancePath+"/va",schemaPath:"#/properties/va/type",keyword:"type",params:{type: schema40.properties.va.type},message:"must be array,null"}];
return false;
}
if(errors === _errs55){
if(Array.isArray(data27)){
var valid15 = true;
const len10 = data27.length;
for(let i10=0; i10<len10; i10++){
const _errs57 = errors;
if(!(validate133(data27[i10], {instancePath:instancePath+"/va/" + i10,parentData:data27,parentDataProperty:i10,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid15 = _errs57 === errors;
if(!valid15){
break;
}
}
}
}
var valid0 = _errs55 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.vm !== undefined){
let data29 = data.vm;
const _errs58 = errors;
if((!(Array.isArray(data29))) && (data29 !== null)){
validate99.errors = [{instancePath:instancePath+"/vm",schemaPath:"#/properties/vm/type",keyword:"type",params:{type: schema40.properties.vm.type},message:"must be array,null"}];
return false;
}
if(errors === _errs58){
if(Array.isArray(data29)){
var valid16 = true;
const len11 = data29.length;
for(let i11=0; i11<len11; i11++){
const _errs60 = errors;
if(!(validate133(data29[i11], {instancePath:instancePath+"/vm/" + i11,parentData:data29,parentDataProperty:i11,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid16 = _errs60 === errors;
if(!valid16){
break;
}
}
}
}
var valid0 = _errs58 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.w !== undefined){
let data31 = data.w;
const _errs61 = errors;
if((!(Array.isArray(data31))) && (data31 !== null)){
validate99.errors = [{instancePath:instancePath+"/w",schemaPath:"#/properties/w/type",keyword:"type",params:{type: schema40.properties.w.type},message:"must be array,null"}];
return false;
}
if(errors === _errs61){
if(Array.isArray(data31)){
var valid17 = true;
const len12 = data31.length;
for(let i12=0; i12<len12; i12++){
const _errs63 = errors;
if(!(validate133(data31[i12], {instancePath:instancePath+"/w/" + i12,parentData:data31,parentDataProperty:i12,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid17 = _errs63 === errors;
if(!valid17){
break;
}
}
}
}
var valid0 = _errs61 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.wi !== undefined){
let data33 = data.wi;
const _errs64 = errors;
if((!(Array.isArray(data33))) && (data33 !== null)){
validate99.errors = [{instancePath:instancePath+"/wi",schemaPath:"#/properties/wi/type",keyword:"type",params:{type: schema40.properties.wi.type},message:"must be array,null"}];
return false;
}
if(errors === _errs64){
if(Array.isArray(data33)){
var valid18 = true;
const len13 = data33.length;
for(let i13=0; i13<len13; i13++){
const _errs66 = errors;
if(!(validate144(data33[i13], {instancePath:instancePath+"/wi/" + i13,parentData:data33,parentDataProperty:i13,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate144.errors : vErrors.concat(validate144.errors);
errors = vErrors.length;
}
var valid18 = _errs66 === errors;
if(!valid18){
break;
}
}
}
}
var valid0 = _errs64 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.wr !== undefined){
let data35 = data.wr;
const _errs67 = errors;
if((!(Array.isArray(data35))) && (data35 !== null)){
validate99.errors = [{instancePath:instancePath+"/wr",schemaPath:"#/properties/wr/type",keyword:"type",params:{type: schema40.properties.wr.type},message:"must be array,null"}];
return false;
}
if(errors === _errs67){
if(Array.isArray(data35)){
var valid19 = true;
const len14 = data35.length;
for(let i14=0; i14<len14; i14++){
const _errs69 = errors;
if(!(validate144(data35[i14], {instancePath:instancePath+"/wr/" + i14,parentData:data35,parentDataProperty:i14,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate144.errors : vErrors.concat(validate144.errors);
errors = vErrors.length;
}
var valid19 = _errs69 === errors;
if(!valid19){
break;
}
}
}
}
var valid0 = _errs67 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate99.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate99.errors = vErrors;
return errors === 0;
}
validate99.evaluated = {"props":{"branch_constraints":true,"branch_susceptance_formula":true,"constraints":true,"diagnostics":true,"dispatch":true,"flows":true,"formulation":true,"initialization":true,"injections":true,"iterations":true,"limit_rounds":true,"lmp":true,"lmp_q":true,"objective":true,"objective_kind":true,"sensitivities":true,"status":true,"va":true,"vm":true,"w":true,"wi":true,"wr":true},"dynamicProps":false,"dynamicItems":false};


function validate333(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate333.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.solution === undefined) && (missing0 = "solution")) || ((data.sensitivities === undefined) && (missing0 = "sensitivities"))){
validate333.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.iterations !== undefined){
let data0 = data.iterations;
const _errs1 = errors;
const _errs2 = errors;
let valid1 = false;
const _errs3 = errors;
if(!(validate60(data0, {instancePath:instancePath+"/iterations",parentData:data,parentDataProperty:"iterations",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate60.errors : vErrors.concat(validate60.errors);
errors = vErrors.length;
}
var _valid0 = _errs3 === errors;
valid1 = valid1 || _valid0;
const _errs4 = errors;
if(data0 !== null){
const err0 = {instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs4 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate333.errors = vErrors;
return false;
}
else {
errors = _errs2;
if(vErrors !== null){
if(_errs2){
vErrors.length = _errs2;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.sensitivities !== undefined){
let data1 = data.sensitivities;
const _errs6 = errors;
if(errors === _errs6){
if(Array.isArray(data1)){
var valid2 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
const _errs8 = errors;
if(!(validate66(data1[i0], {instancePath:instancePath+"/sensitivities/" + i0,parentData:data1,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate66.errors : vErrors.concat(validate66.errors);
errors = vErrors.length;
}
var valid2 = _errs8 === errors;
if(!valid2){
break;
}
}
}
else {
validate333.errors = [{instancePath:instancePath+"/sensitivities",schemaPath:"#/properties/sensitivities/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.solution !== undefined){
const _errs9 = errors;
if(!(validate99(data.solution, {instancePath:instancePath+"/solution",parentData:data,parentDataProperty:"solution",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate99.errors : vErrors.concat(validate99.errors);
errors = vErrors.length;
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate333.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate333.errors = vErrors;
return errors === 0;
}
validate333.evaluated = {"props":{"iterations":true,"sensitivities":true,"solution":true},"dynamicProps":false,"dynamicItems":false};

export const edits = validate337;
const schema61 = {"description":"Operating-point edits applied before the model is built: demand deltas in MW\nkeyed by bus (the operating point is `base demand + delta`) and branch rating\ndeltas in MW keyed by branch (the thermal limit is `base rating + delta`). Keys\nare [`ElementKey`]s — the original numeric id or the powerio row uid. A struct\n(not a bare map) so the structural-edit vocabulary (add line, add generator,\nretune a parameter) can grow without breaking the wire format: a client that\nknows only `deltas` keeps working.","properties":{"deltas":{"additionalProperties":{"format":"double","type":"number"},"default":{},"description":"Active-power demand delta in MW per bus key.","type":"object"},"rates":{"additionalProperties":{"format":"double","type":"number"},"default":{},"description":"Thermal rating delta in MW per branch key. Supported by the DC OPF\nand SOCWR paths; the AC power flow has no flow limits and rejects them.","type":"object"}},"type":"object"};

function validate337(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate337.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.deltas !== undefined){
let data0 = data.deltas;
const _errs1 = errors;
if(errors === _errs1){
if(data0 && typeof data0 == "object" && !Array.isArray(data0)){
for(const key0 in data0){
let data1 = data0[key0];
const _errs4 = errors;
if(errors === _errs4){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate337.errors = [{instancePath:instancePath+"/deltas/" + key0.replace(/~/g, "~0").replace(/\//g, "~1"),schemaPath:"#/properties/deltas/additionalProperties/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid1 = _errs4 === errors;
if(!valid1){
break;
}
}
}
else {
validate337.errors = [{instancePath:instancePath+"/deltas",schemaPath:"#/properties/deltas/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rates !== undefined){
let data2 = data.rates;
const _errs6 = errors;
if(errors === _errs6){
if(data2 && typeof data2 == "object" && !Array.isArray(data2)){
for(const key1 in data2){
let data3 = data2[key1];
const _errs9 = errors;
if(errors === _errs9){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate337.errors = [{instancePath:instancePath+"/rates/" + key1.replace(/~/g, "~0").replace(/\//g, "~1"),schemaPath:"#/properties/rates/additionalProperties/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid2 = _errs9 === errors;
if(!valid2){
break;
}
}
}
else {
validate337.errors = [{instancePath:instancePath+"/rates",schemaPath:"#/properties/rates/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
}
}
else {
validate337.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate337.errors = vErrors;
return errors === 0;
}
validate337.evaluated = {"props":{"deltas":true,"rates":true},"dynamicProps":false,"dynamicItems":false};

export const mc_detail = validate338;
const schema62 = {"description":"One page of detailed values, in the same records as [`McPfResult`].","properties":{"element_port_total":{"description":"Ports matching the query before paging.","format":"uint","minimum":0,"type":"integer"},"element_ports":{"items":{"$ref":"#/$defs/McElementPort"},"type":"array"},"solve_count":{"format":"uint","minimum":0,"type":"integer"},"terminals":{"items":{"$ref":"#/$defs/McTerminalResult"},"type":"array"}},"required":["solve_count","terminals","element_ports","element_port_total"],"type":"object"};
const schema63 = {"properties":{"branch":{"format":"uint","minimum":0,"type":"integer"},"bus":{"type":"string"},"current_into_element":{"$ref":"#/$defs/McComplex"},"element":{"type":"string"},"kind":{"type":"string"},"power_into_element":{"$ref":"#/$defs/McComplex"},"terminal":{"type":"string"}},"required":["element","kind","branch","bus","terminal","current_into_element","power_into_element"],"type":"object"};
const schema64 = {"properties":{"im":{"format":"double","type":"number"},"re":{"format":"double","type":"number"}},"required":["re","im"],"type":"object"};

function validate153(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate153.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.re === undefined) && (missing0 = "re")) || ((data.im === undefined) && (missing0 = "im"))){
validate153.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.im !== undefined){
let data0 = data.im;
const _errs1 = errors;
if(errors === _errs1){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate153.errors = [{instancePath:instancePath+"/im",schemaPath:"#/properties/im/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.re !== undefined){
let data1 = data.re;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate153.errors = [{instancePath:instancePath+"/re",schemaPath:"#/properties/re/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate153.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate153.errors = vErrors;
return errors === 0;
}
validate153.evaluated = {"props":{"im":true,"re":true},"dynamicProps":false,"dynamicItems":false};


function validate152(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate152.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.element === undefined) && (missing0 = "element")) || ((data.kind === undefined) && (missing0 = "kind"))) || ((data.branch === undefined) && (missing0 = "branch"))) || ((data.bus === undefined) && (missing0 = "bus"))) || ((data.terminal === undefined) && (missing0 = "terminal"))) || ((data.current_into_element === undefined) && (missing0 = "current_into_element"))) || ((data.power_into_element === undefined) && (missing0 = "power_into_element"))){
validate152.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch !== undefined){
let data0 = data.branch;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate152.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate152.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.bus !== undefined){
const _errs3 = errors;
if(typeof data.bus !== "string"){
validate152.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.current_into_element !== undefined){
const _errs5 = errors;
if(!(validate153(data.current_into_element, {instancePath:instancePath+"/current_into_element",parentData:data,parentDataProperty:"current_into_element",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.element !== undefined){
const _errs6 = errors;
if(typeof data.element !== "string"){
validate152.errors = [{instancePath:instancePath+"/element",schemaPath:"#/properties/element/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.kind !== undefined){
const _errs8 = errors;
if(typeof data.kind !== "string"){
validate152.errors = [{instancePath:instancePath+"/kind",schemaPath:"#/properties/kind/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.power_into_element !== undefined){
const _errs10 = errors;
if(!(validate153(data.power_into_element, {instancePath:instancePath+"/power_into_element",parentData:data,parentDataProperty:"power_into_element",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.terminal !== undefined){
const _errs11 = errors;
if(typeof data.terminal !== "string"){
validate152.errors = [{instancePath:instancePath+"/terminal",schemaPath:"#/properties/terminal/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate152.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate152.errors = vErrors;
return errors === 0;
}
validate152.evaluated = {"props":{"branch":true,"bus":true,"current_into_element":true,"element":true,"kind":true,"power_into_element":true,"terminal":true},"dynamicProps":false,"dynamicItems":false};

const schema65 = {"properties":{"bus":{"type":"string"},"current_into_network":{"$ref":"#/$defs/McComplex","description":"Current injected into the network at this terminal, amperes."},"power_into_network":{"$ref":"#/$defs/McComplex","description":"Terminal complex power using `V * conj(I_into_network)`, VA."},"terminal":{"type":"string"},"voltage":{"$ref":"#/$defs/McComplex"}},"required":["bus","terminal","voltage","current_into_network","power_into_network"],"type":"object"};

function validate157(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate157.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((data.bus === undefined) && (missing0 = "bus")) || ((data.terminal === undefined) && (missing0 = "terminal"))) || ((data.voltage === undefined) && (missing0 = "voltage"))) || ((data.current_into_network === undefined) && (missing0 = "current_into_network"))) || ((data.power_into_network === undefined) && (missing0 = "power_into_network"))){
validate157.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
const _errs1 = errors;
if(typeof data.bus !== "string"){
validate157.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.current_into_network !== undefined){
const _errs3 = errors;
if(!(validate153(data.current_into_network, {instancePath:instancePath+"/current_into_network",parentData:data,parentDataProperty:"current_into_network",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.power_into_network !== undefined){
const _errs4 = errors;
if(!(validate153(data.power_into_network, {instancePath:instancePath+"/power_into_network",parentData:data,parentDataProperty:"power_into_network",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.terminal !== undefined){
const _errs5 = errors;
if(typeof data.terminal !== "string"){
validate157.errors = [{instancePath:instancePath+"/terminal",schemaPath:"#/properties/terminal/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage !== undefined){
const _errs7 = errors;
if(!(validate153(data.voltage, {instancePath:instancePath+"/voltage",parentData:data,parentDataProperty:"voltage",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
else {
validate157.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate157.errors = vErrors;
return errors === 0;
}
validate157.evaluated = {"props":{"bus":true,"current_into_network":true,"power_into_network":true,"terminal":true,"voltage":true},"dynamicProps":false,"dynamicItems":false};


function validate338(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate338.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((data.solve_count === undefined) && (missing0 = "solve_count")) || ((data.terminals === undefined) && (missing0 = "terminals"))) || ((data.element_ports === undefined) && (missing0 = "element_ports"))) || ((data.element_port_total === undefined) && (missing0 = "element_port_total"))){
validate338.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.element_port_total !== undefined){
let data0 = data.element_port_total;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate338.errors = [{instancePath:instancePath+"/element_port_total",schemaPath:"#/properties/element_port_total/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate338.errors = [{instancePath:instancePath+"/element_port_total",schemaPath:"#/properties/element_port_total/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.element_ports !== undefined){
let data1 = data.element_ports;
const _errs3 = errors;
if(errors === _errs3){
if(Array.isArray(data1)){
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
const _errs5 = errors;
if(!(validate152(data1[i0], {instancePath:instancePath+"/element_ports/" + i0,parentData:data1,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate152.errors : vErrors.concat(validate152.errors);
errors = vErrors.length;
}
var valid1 = _errs5 === errors;
if(!valid1){
break;
}
}
}
else {
validate338.errors = [{instancePath:instancePath+"/element_ports",schemaPath:"#/properties/element_ports/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.solve_count !== undefined){
let data3 = data.solve_count;
const _errs6 = errors;
if(!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))){
validate338.errors = [{instancePath:instancePath+"/solve_count",schemaPath:"#/properties/solve_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs6){
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 0 || isNaN(data3)){
validate338.errors = [{instancePath:instancePath+"/solve_count",schemaPath:"#/properties/solve_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.terminals !== undefined){
let data4 = data.terminals;
const _errs8 = errors;
if(errors === _errs8){
if(Array.isArray(data4)){
var valid2 = true;
const len1 = data4.length;
for(let i1=0; i1<len1; i1++){
const _errs10 = errors;
if(!(validate157(data4[i1], {instancePath:instancePath+"/terminals/" + i1,parentData:data4,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate157.errors : vErrors.concat(validate157.errors);
errors = vErrors.length;
}
var valid2 = _errs10 === errors;
if(!valid2){
break;
}
}
}
else {
validate338.errors = [{instancePath:instancePath+"/terminals",schemaPath:"#/properties/terminals/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate338.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate338.errors = vErrors;
return errors === 0;
}
validate338.evaluated = {"props":{"element_port_total":true,"element_ports":true,"solve_count":true,"terminals":true},"dynamicProps":false,"dynamicItems":false};

export const mc_load_branches = validate341;
const schema129 = {"items":{"$ref":"#/$defs/McLoadBranchState"},"type":"array"};
const schema66 = {"description":"UI-facing identity and current/base power for one editable load branch.","properties":{"base_p_w":{"format":"double","type":"number"},"base_q_var":{"format":"double","type":"number"},"branch":{"format":"uint","minimum":0,"type":"integer"},"bus":{"type":"string"},"load":{"type":"string"},"p_w":{"format":"double","type":"number"},"q_var":{"format":"double","type":"number"}},"required":["load","bus","branch","p_w","q_var","base_p_w","base_q_var"],"type":"object"};

function validate163(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate163.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.load === undefined) && (missing0 = "load")) || ((data.bus === undefined) && (missing0 = "bus"))) || ((data.branch === undefined) && (missing0 = "branch"))) || ((data.p_w === undefined) && (missing0 = "p_w"))) || ((data.q_var === undefined) && (missing0 = "q_var"))) || ((data.base_p_w === undefined) && (missing0 = "base_p_w"))) || ((data.base_q_var === undefined) && (missing0 = "base_q_var"))){
validate163.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.base_p_w !== undefined){
let data0 = data.base_p_w;
const _errs1 = errors;
if(errors === _errs1){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate163.errors = [{instancePath:instancePath+"/base_p_w",schemaPath:"#/properties/base_p_w/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.base_q_var !== undefined){
let data1 = data.base_q_var;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate163.errors = [{instancePath:instancePath+"/base_q_var",schemaPath:"#/properties/base_q_var/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.branch !== undefined){
let data2 = data.branch;
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate163.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate163.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.bus !== undefined){
const _errs7 = errors;
if(typeof data.bus !== "string"){
validate163.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.load !== undefined){
const _errs9 = errors;
if(typeof data.load !== "string"){
validate163.errors = [{instancePath:instancePath+"/load",schemaPath:"#/properties/load/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.p_w !== undefined){
let data5 = data.p_w;
const _errs11 = errors;
if(errors === _errs11){
if(!((typeof data5 == "number") && (isFinite(data5)))){
validate163.errors = [{instancePath:instancePath+"/p_w",schemaPath:"#/properties/p_w/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.q_var !== undefined){
let data6 = data.q_var;
const _errs13 = errors;
if(errors === _errs13){
if(!((typeof data6 == "number") && (isFinite(data6)))){
validate163.errors = [{instancePath:instancePath+"/q_var",schemaPath:"#/properties/q_var/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate163.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate163.errors = vErrors;
return errors === 0;
}
validate163.evaluated = {"props":{"base_p_w":true,"base_q_var":true,"branch":true,"bus":true,"load":true,"p_w":true,"q_var":true},"dynamicProps":false,"dynamicItems":false};


function validate341(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate341.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(Array.isArray(data)){
var valid0 = true;
const len0 = data.length;
for(let i0=0; i0<len0; i0++){
const _errs1 = errors;
if(!(validate163(data[i0], {instancePath:instancePath+"/" + i0,parentData:data,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate163.errors : vErrors.concat(validate163.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
if(!valid0){
break;
}
}
}
else {
validate341.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
validate341.errors = vErrors;
return errors === 0;
}
validate341.evaluated = {"items":true,"dynamicProps":false,"dynamicItems":false};

export const mc_profile = validate343;
const schema67 = {"description":"Where one solve spent its time, in milliseconds of the profile clock.\n\nA phase that did not run is zero. `timed` is false when no clock is\navailable (a WASM build without an injected clock), in which case every\nduration is zero.","properties":{"factor_ms":{"description":"Sparse assembly and numeric LU factorization (cold start only).","format":"double","type":"number"},"iterations":{"format":"uint","minimum":0,"type":"integer"},"kcl_and_matvec_ms":{"description":"Compensated right hand sides and KCL residual products.","format":"double","type":"number"},"linear_solve_ms":{"description":"Retained sparse LU solves.","format":"double","type":"number"},"linear_solves":{"format":"uint","minimum":0,"type":"integer"},"load_evaluation_ms":{"description":"Load and device current evaluation.","format":"double","type":"number"},"parse_ms":{"description":"Parsing the stored module (cold start from a module only).","format":"double","type":"number"},"prepare_ms":{"description":"Terminal indexing, stamping, and load preparation (cold start only).","format":"double","type":"number"},"summary_ms":{"description":"Summary aggregation and output finiteness checks.","format":"double","type":"number"},"timed":{"type":"boolean"},"total_ms":{"format":"double","type":"number"}},"required":["timed","parse_ms","prepare_ms","factor_ms","load_evaluation_ms","kcl_and_matvec_ms","linear_solve_ms","summary_ms","total_ms","iterations","linear_solves"],"type":"object"};

function validate343(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate343.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((((((data.timed === undefined) && (missing0 = "timed")) || ((data.parse_ms === undefined) && (missing0 = "parse_ms"))) || ((data.prepare_ms === undefined) && (missing0 = "prepare_ms"))) || ((data.factor_ms === undefined) && (missing0 = "factor_ms"))) || ((data.load_evaluation_ms === undefined) && (missing0 = "load_evaluation_ms"))) || ((data.kcl_and_matvec_ms === undefined) && (missing0 = "kcl_and_matvec_ms"))) || ((data.linear_solve_ms === undefined) && (missing0 = "linear_solve_ms"))) || ((data.summary_ms === undefined) && (missing0 = "summary_ms"))) || ((data.total_ms === undefined) && (missing0 = "total_ms"))) || ((data.iterations === undefined) && (missing0 = "iterations"))) || ((data.linear_solves === undefined) && (missing0 = "linear_solves"))){
validate343.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.factor_ms !== undefined){
let data0 = data.factor_ms;
const _errs1 = errors;
if(errors === _errs1){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate343.errors = [{instancePath:instancePath+"/factor_ms",schemaPath:"#/properties/factor_ms/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iterations !== undefined){
let data1 = data.iterations;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate343.errors = [{instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate343.errors = [{instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.kcl_and_matvec_ms !== undefined){
let data2 = data.kcl_and_matvec_ms;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate343.errors = [{instancePath:instancePath+"/kcl_and_matvec_ms",schemaPath:"#/properties/kcl_and_matvec_ms/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.linear_solve_ms !== undefined){
let data3 = data.linear_solve_ms;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate343.errors = [{instancePath:instancePath+"/linear_solve_ms",schemaPath:"#/properties/linear_solve_ms/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.linear_solves !== undefined){
let data4 = data.linear_solves;
const _errs9 = errors;
if(!(((typeof data4 == "number") && (!(data4 % 1) && !isNaN(data4))) && (isFinite(data4)))){
validate343.errors = [{instancePath:instancePath+"/linear_solves",schemaPath:"#/properties/linear_solves/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs9){
if((typeof data4 == "number") && (isFinite(data4))){
if(data4 < 0 || isNaN(data4)){
validate343.errors = [{instancePath:instancePath+"/linear_solves",schemaPath:"#/properties/linear_solves/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.load_evaluation_ms !== undefined){
let data5 = data.load_evaluation_ms;
const _errs11 = errors;
if(errors === _errs11){
if(!((typeof data5 == "number") && (isFinite(data5)))){
validate343.errors = [{instancePath:instancePath+"/load_evaluation_ms",schemaPath:"#/properties/load_evaluation_ms/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.parse_ms !== undefined){
let data6 = data.parse_ms;
const _errs13 = errors;
if(errors === _errs13){
if(!((typeof data6 == "number") && (isFinite(data6)))){
validate343.errors = [{instancePath:instancePath+"/parse_ms",schemaPath:"#/properties/parse_ms/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.prepare_ms !== undefined){
let data7 = data.prepare_ms;
const _errs15 = errors;
if(errors === _errs15){
if(!((typeof data7 == "number") && (isFinite(data7)))){
validate343.errors = [{instancePath:instancePath+"/prepare_ms",schemaPath:"#/properties/prepare_ms/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.summary_ms !== undefined){
let data8 = data.summary_ms;
const _errs17 = errors;
if(errors === _errs17){
if(!((typeof data8 == "number") && (isFinite(data8)))){
validate343.errors = [{instancePath:instancePath+"/summary_ms",schemaPath:"#/properties/summary_ms/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs17 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.timed !== undefined){
const _errs19 = errors;
if(typeof data.timed !== "boolean"){
validate343.errors = [{instancePath:instancePath+"/timed",schemaPath:"#/properties/timed/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs19 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.total_ms !== undefined){
let data10 = data.total_ms;
const _errs21 = errors;
if(errors === _errs21){
if(!((typeof data10 == "number") && (isFinite(data10)))){
validate343.errors = [{instancePath:instancePath+"/total_ms",schemaPath:"#/properties/total_ms/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate343.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate343.errors = vErrors;
return errors === 0;
}
validate343.evaluated = {"props":{"factor_ms":true,"iterations":true,"kcl_and_matvec_ms":true,"linear_solve_ms":true,"linear_solves":true,"load_evaluation_ms":true,"parse_ms":true,"prepare_ms":true,"summary_ms":true,"timed":true,"total_ms":true},"dynamicProps":false,"dynamicItems":false};

export const mc_result = validate344;
const schema68 = {"properties":{"converged":{"type":"boolean"},"element_ports":{"items":{"$ref":"#/$defs/McElementPort"},"type":"array"},"excluded_islands":{"items":{"$ref":"#/$defs/McExcludedIsland"},"type":"array"},"factorization_count":{"format":"uint","minimum":0,"type":"integer"},"iterations":{"format":"uint","minimum":0,"type":"integer"},"load_model":{"anyOf":[{"$ref":"#/$defs/McLoadModelAssessment"},{"type":"null"}],"default":null,"description":"Absent only on legacy results, whose model fidelity is not yet assessed."},"load_model_changes":{"default":[],"items":{"$ref":"#/$defs/McLoadModelChange"},"type":"array"},"matrix_dimension":{"format":"uint","minimum":0,"type":"integer"},"matrix_nonzeros":{"format":"uint","minimum":0,"type":"integer"},"max_voltage_pu":{"format":"double","type":["number","null"]},"min_voltage_pu":{"format":"double","type":["number","null"]},"network_scope":{"anyOf":[{"$ref":"#/$defs/McNetworkScope"},{"type":"null"}]},"physical_kcl_residual":{"format":"double","type":"number"},"scaled_kcl_residual":{"format":"double","type":"number"},"source_reactions":{"items":{"$ref":"#/$defs/McSourceReaction"},"type":"array"},"terminals":{"items":{"$ref":"#/$defs/McTerminalResult"},"type":"array"},"voltage_change":{"format":"double","type":"number"},"voltage_valid":{"description":"Whether every load branch lies inside the configured normal voltage band.","type":"boolean"},"voltage_violations":{"items":{"$ref":"#/$defs/McVoltageViolation"},"type":"array"}},"required":["converged","voltage_valid","voltage_violations","iterations","factorization_count","matrix_dimension","matrix_nonzeros","voltage_change","physical_kcl_residual","scaled_kcl_residual","terminals","element_ports","source_reactions"],"type":"object"};
const schema69 = {"properties":{"buses":{"items":{"type":"string"},"type":"array"},"equipment":{"description":"Includes open boundary switches touching this island. An open switch\nbetween two excluded islands appears in both island records.","items":{"$ref":"#/$defs/McExcludedEquipment"},"type":"array"}},"required":["buses","equipment"],"type":"object"};
const schema70 = {"properties":{"kind":{"type":"string"},"name":{"type":"string"}},"required":["kind","name"],"type":"object"};

function validate170(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate170.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.kind === undefined) && (missing0 = "kind")) || ((data.name === undefined) && (missing0 = "name"))){
validate170.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.kind !== undefined){
const _errs1 = errors;
if(typeof data.kind !== "string"){
validate170.errors = [{instancePath:instancePath+"/kind",schemaPath:"#/properties/kind/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.name !== undefined){
const _errs3 = errors;
if(typeof data.name !== "string"){
validate170.errors = [{instancePath:instancePath+"/name",schemaPath:"#/properties/name/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate170.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate170.errors = vErrors;
return errors === 0;
}
validate170.evaluated = {"props":{"kind":true,"name":true},"dynamicProps":false,"dynamicItems":false};


function validate169(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate169.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.buses === undefined) && (missing0 = "buses")) || ((data.equipment === undefined) && (missing0 = "equipment"))){
validate169.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.buses !== undefined){
let data0 = data.buses;
const _errs1 = errors;
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(typeof data0[i0] !== "string"){
validate169.errors = [{instancePath:instancePath+"/buses/" + i0,schemaPath:"#/properties/buses/items/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
else {
validate169.errors = [{instancePath:instancePath+"/buses",schemaPath:"#/properties/buses/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.equipment !== undefined){
let data2 = data.equipment;
const _errs5 = errors;
if(errors === _errs5){
if(Array.isArray(data2)){
var valid2 = true;
const len1 = data2.length;
for(let i1=0; i1<len1; i1++){
const _errs7 = errors;
if(!(validate170(data2[i1], {instancePath:instancePath+"/equipment/" + i1,parentData:data2,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate170.errors : vErrors.concat(validate170.errors);
errors = vErrors.length;
}
var valid2 = _errs7 === errors;
if(!valid2){
break;
}
}
}
else {
validate169.errors = [{instancePath:instancePath+"/equipment",schemaPath:"#/properties/equipment/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate169.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate169.errors = vErrors;
return errors === 0;
}
validate169.evaluated = {"props":{"buses":true,"equipment":true},"dynamicProps":false,"dynamicItems":false};

const schema71 = {"description":"Constant-size provenance carried by summaries and complete results.","properties":{"modified_branch_count":{"format":"uint","minimum":0,"type":"integer"},"options":{"$ref":"#/$defs/McPfOptions"},"original_kcl_residual":{"description":"None when an original load law is undefined or non-finite at this point.","format":"double","type":["number","null"]},"original_scaled_kcl_residual":{"format":"double","type":["number","null"]},"status":{"$ref":"#/$defs/McLoadModelStatus"}},"required":["status","modified_branch_count","options"],"type":"object"};
const schema72 = {"properties":{"absolute_kcl_tolerance":{"default":0.0001,"format":"double","type":"number"},"damping":{"default":1,"format":"double","type":"number"},"max_iterations":{"default":100,"format":"uint","minimum":0,"type":"integer"},"relative_kcl_tolerance":{"default":0.00001,"format":"double","type":"number"},"tolerance":{"default":0.0001,"format":"double","type":"number"},"v_low_pu":{"default":0.5,"description":"Below this per-unit voltage, loads use their nominal impedance.","format":"double","type":"number"},"v_max_pu":{"default":1.15,"description":"Upper edge of the load model's normal voltage range.","format":"double","type":"number"},"v_min_pu":{"default":0.85,"description":"Lower edge of the load model's normal voltage range.","format":"double","type":"number"},"voltage_envelope":{"default":true,"description":"Apply the legacy OpenDSS-style envelope to unbounded load laws.\nExplicit PowerIO bounded laws always retain their own thresholds,\ncoefficients, cutoff and current limit regardless of this option.","type":"boolean"},"zero_voltage_tolerance":{"default":1e-9,"format":"double","type":"number"}},"type":"object"};

function validate174(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate174.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
if(data.absolute_kcl_tolerance !== undefined){
let data0 = data.absolute_kcl_tolerance;
const _errs1 = errors;
if(errors === _errs1){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate174.errors = [{instancePath:instancePath+"/absolute_kcl_tolerance",schemaPath:"#/properties/absolute_kcl_tolerance/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.damping !== undefined){
let data1 = data.damping;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate174.errors = [{instancePath:instancePath+"/damping",schemaPath:"#/properties/damping/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.max_iterations !== undefined){
let data2 = data.max_iterations;
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate174.errors = [{instancePath:instancePath+"/max_iterations",schemaPath:"#/properties/max_iterations/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate174.errors = [{instancePath:instancePath+"/max_iterations",schemaPath:"#/properties/max_iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.relative_kcl_tolerance !== undefined){
let data3 = data.relative_kcl_tolerance;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate174.errors = [{instancePath:instancePath+"/relative_kcl_tolerance",schemaPath:"#/properties/relative_kcl_tolerance/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.tolerance !== undefined){
let data4 = data.tolerance;
const _errs9 = errors;
if(errors === _errs9){
if(!((typeof data4 == "number") && (isFinite(data4)))){
validate174.errors = [{instancePath:instancePath+"/tolerance",schemaPath:"#/properties/tolerance/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.v_low_pu !== undefined){
let data5 = data.v_low_pu;
const _errs11 = errors;
if(errors === _errs11){
if(!((typeof data5 == "number") && (isFinite(data5)))){
validate174.errors = [{instancePath:instancePath+"/v_low_pu",schemaPath:"#/properties/v_low_pu/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.v_max_pu !== undefined){
let data6 = data.v_max_pu;
const _errs13 = errors;
if(errors === _errs13){
if(!((typeof data6 == "number") && (isFinite(data6)))){
validate174.errors = [{instancePath:instancePath+"/v_max_pu",schemaPath:"#/properties/v_max_pu/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.v_min_pu !== undefined){
let data7 = data.v_min_pu;
const _errs15 = errors;
if(errors === _errs15){
if(!((typeof data7 == "number") && (isFinite(data7)))){
validate174.errors = [{instancePath:instancePath+"/v_min_pu",schemaPath:"#/properties/v_min_pu/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_envelope !== undefined){
const _errs17 = errors;
if(typeof data.voltage_envelope !== "boolean"){
validate174.errors = [{instancePath:instancePath+"/voltage_envelope",schemaPath:"#/properties/voltage_envelope/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs17 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.zero_voltage_tolerance !== undefined){
let data9 = data.zero_voltage_tolerance;
const _errs19 = errors;
if(errors === _errs19){
if(!((typeof data9 == "number") && (isFinite(data9)))){
validate174.errors = [{instancePath:instancePath+"/zero_voltage_tolerance",schemaPath:"#/properties/zero_voltage_tolerance/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs19 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
else {
validate174.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate174.errors = vErrors;
return errors === 0;
}
validate174.evaluated = {"props":{"absolute_kcl_tolerance":true,"damping":true,"max_iterations":true,"relative_kcl_tolerance":true,"tolerance":true,"v_low_pu":true,"v_max_pu":true,"v_min_pu":true,"voltage_envelope":true,"zero_voltage_tolerance":true},"dynamicProps":false,"dynamicItems":false};

const schema73 = {"description":"Qualification of the final operating point, independent of voltage limits.","enum":["original","modified","unverified"],"type":"string"};

function validate176(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate176.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate176.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!(((data === "original") || (data === "modified")) || (data === "unverified"))){
validate176.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema73.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate176.errors = vErrors;
return errors === 0;
}
validate176.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate173(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate173.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.status === undefined) && (missing0 = "status")) || ((data.modified_branch_count === undefined) && (missing0 = "modified_branch_count"))) || ((data.options === undefined) && (missing0 = "options"))){
validate173.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.modified_branch_count !== undefined){
let data0 = data.modified_branch_count;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate173.errors = [{instancePath:instancePath+"/modified_branch_count",schemaPath:"#/properties/modified_branch_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate173.errors = [{instancePath:instancePath+"/modified_branch_count",schemaPath:"#/properties/modified_branch_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.options !== undefined){
const _errs3 = errors;
if(!(validate174(data.options, {instancePath:instancePath+"/options",parentData:data,parentDataProperty:"options",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate174.errors : vErrors.concat(validate174.errors);
errors = vErrors.length;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.original_kcl_residual !== undefined){
let data2 = data.original_kcl_residual;
const _errs4 = errors;
if((!((typeof data2 == "number") && (isFinite(data2)))) && (data2 !== null)){
validate173.errors = [{instancePath:instancePath+"/original_kcl_residual",schemaPath:"#/properties/original_kcl_residual/type",keyword:"type",params:{type: schema71.properties.original_kcl_residual.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.original_scaled_kcl_residual !== undefined){
let data3 = data.original_scaled_kcl_residual;
const _errs6 = errors;
if((!((typeof data3 == "number") && (isFinite(data3)))) && (data3 !== null)){
validate173.errors = [{instancePath:instancePath+"/original_scaled_kcl_residual",schemaPath:"#/properties/original_scaled_kcl_residual/type",keyword:"type",params:{type: schema71.properties.original_scaled_kcl_residual.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.status !== undefined){
const _errs8 = errors;
if(!(validate176(data.status, {instancePath:instancePath+"/status",parentData:data,parentDataProperty:"status",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate176.errors : vErrors.concat(validate176.errors);
errors = vErrors.length;
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
else {
validate173.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate173.errors = vErrors;
return errors === 0;
}
validate173.evaluated = {"props":{"modified_branch_count":true,"options":true,"original_kcl_residual":true,"original_scaled_kcl_residual":true,"status":true},"dynamicProps":false,"dynamicItems":false};

const schema74 = {"description":"A branch whose final current differs from its original load law.","properties":{"branch":{"format":"uint","minimum":0,"type":"integer"},"bus":{"type":"string"},"load":{"type":"string"},"original_power":{"anyOf":[{"$ref":"#/$defs/McComplex"},{"type":"null"}],"description":"Original-law power at the same voltage; None if the law is undefined."},"solved_power":{"$ref":"#/$defs/McComplex","description":"Actual absorbed power, W + j var."},"voltage_pu":{"format":"double","type":"number"}},"required":["load","branch","bus","voltage_pu","solved_power"],"type":"object"};

function validate179(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate179.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((data.load === undefined) && (missing0 = "load")) || ((data.branch === undefined) && (missing0 = "branch"))) || ((data.bus === undefined) && (missing0 = "bus"))) || ((data.voltage_pu === undefined) && (missing0 = "voltage_pu"))) || ((data.solved_power === undefined) && (missing0 = "solved_power"))){
validate179.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch !== undefined){
let data0 = data.branch;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate179.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate179.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.bus !== undefined){
const _errs3 = errors;
if(typeof data.bus !== "string"){
validate179.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.load !== undefined){
const _errs5 = errors;
if(typeof data.load !== "string"){
validate179.errors = [{instancePath:instancePath+"/load",schemaPath:"#/properties/load/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.original_power !== undefined){
let data3 = data.original_power;
const _errs7 = errors;
const _errs8 = errors;
let valid1 = false;
const _errs9 = errors;
if(!(validate153(data3, {instancePath:instancePath+"/original_power",parentData:data,parentDataProperty:"original_power",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var _valid0 = _errs9 === errors;
valid1 = valid1 || _valid0;
if(_valid0){
var props0 = {};
props0.im = true;
props0.re = true;
}
const _errs10 = errors;
if(data3 !== null){
const err0 = {instancePath:instancePath+"/original_power",schemaPath:"#/properties/original_power/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs10 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/original_power",schemaPath:"#/properties/original_power/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate179.errors = vErrors;
return false;
}
else {
errors = _errs8;
if(vErrors !== null){
if(_errs8){
vErrors.length = _errs8;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.solved_power !== undefined){
const _errs12 = errors;
if(!(validate153(data.solved_power, {instancePath:instancePath+"/solved_power",parentData:data,parentDataProperty:"solved_power",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_pu !== undefined){
let data5 = data.voltage_pu;
const _errs13 = errors;
if(errors === _errs13){
if(!((typeof data5 == "number") && (isFinite(data5)))){
validate179.errors = [{instancePath:instancePath+"/voltage_pu",schemaPath:"#/properties/voltage_pu/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
else {
validate179.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate179.errors = vErrors;
return errors === 0;
}
validate179.evaluated = {"props":{"branch":true,"bus":true,"load":true,"original_power":true,"solved_power":true,"voltage_pu":true},"dynamicProps":false,"dynamicItems":false};

const schema75 = {"properties":{"excluded_bus_count":{"format":"uint","minimum":0,"type":"integer"},"excluded_equipment_count":{"format":"uint","minimum":0,"type":"integer"},"excluded_island_count":{"format":"uint","minimum":0,"type":"integer"},"excluded_load_count":{"format":"uint","minimum":0,"type":"integer"}},"required":["excluded_island_count","excluded_bus_count","excluded_equipment_count","excluded_load_count"],"type":"object"};

function validate183(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate183.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((data.excluded_island_count === undefined) && (missing0 = "excluded_island_count")) || ((data.excluded_bus_count === undefined) && (missing0 = "excluded_bus_count"))) || ((data.excluded_equipment_count === undefined) && (missing0 = "excluded_equipment_count"))) || ((data.excluded_load_count === undefined) && (missing0 = "excluded_load_count"))){
validate183.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.excluded_bus_count !== undefined){
let data0 = data.excluded_bus_count;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate183.errors = [{instancePath:instancePath+"/excluded_bus_count",schemaPath:"#/properties/excluded_bus_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate183.errors = [{instancePath:instancePath+"/excluded_bus_count",schemaPath:"#/properties/excluded_bus_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.excluded_equipment_count !== undefined){
let data1 = data.excluded_equipment_count;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate183.errors = [{instancePath:instancePath+"/excluded_equipment_count",schemaPath:"#/properties/excluded_equipment_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate183.errors = [{instancePath:instancePath+"/excluded_equipment_count",schemaPath:"#/properties/excluded_equipment_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.excluded_island_count !== undefined){
let data2 = data.excluded_island_count;
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate183.errors = [{instancePath:instancePath+"/excluded_island_count",schemaPath:"#/properties/excluded_island_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate183.errors = [{instancePath:instancePath+"/excluded_island_count",schemaPath:"#/properties/excluded_island_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.excluded_load_count !== undefined){
let data3 = data.excluded_load_count;
const _errs7 = errors;
if(!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))){
validate183.errors = [{instancePath:instancePath+"/excluded_load_count",schemaPath:"#/properties/excluded_load_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs7){
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 0 || isNaN(data3)){
validate183.errors = [{instancePath:instancePath+"/excluded_load_count",schemaPath:"#/properties/excluded_load_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate183.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate183.errors = vErrors;
return errors === 0;
}
validate183.evaluated = {"props":{"excluded_bus_count":true,"excluded_equipment_count":true,"excluded_island_count":true,"excluded_load_count":true},"dynamicProps":false,"dynamicItems":false};

const schema76 = {"properties":{"current_into_network":{"$ref":"#/$defs/McComplex"},"power_into_network":{"$ref":"#/$defs/McComplex"},"source":{"type":"string"},"terminal":{"type":"string"}},"required":["source","terminal","current_into_network","power_into_network"],"type":"object"};

function validate185(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate185.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((data.source === undefined) && (missing0 = "source")) || ((data.terminal === undefined) && (missing0 = "terminal"))) || ((data.current_into_network === undefined) && (missing0 = "current_into_network"))) || ((data.power_into_network === undefined) && (missing0 = "power_into_network"))){
validate185.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.current_into_network !== undefined){
const _errs1 = errors;
if(!(validate153(data.current_into_network, {instancePath:instancePath+"/current_into_network",parentData:data,parentDataProperty:"current_into_network",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.power_into_network !== undefined){
const _errs2 = errors;
if(!(validate153(data.power_into_network, {instancePath:instancePath+"/power_into_network",parentData:data,parentDataProperty:"power_into_network",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.source !== undefined){
const _errs3 = errors;
if(typeof data.source !== "string"){
validate185.errors = [{instancePath:instancePath+"/source",schemaPath:"#/properties/source/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.terminal !== undefined){
const _errs5 = errors;
if(typeof data.terminal !== "string"){
validate185.errors = [{instancePath:instancePath+"/terminal",schemaPath:"#/properties/terminal/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate185.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate185.errors = vErrors;
return errors === 0;
}
validate185.evaluated = {"props":{"current_into_network":true,"power_into_network":true,"source":true,"terminal":true},"dynamicProps":false,"dynamicItems":false};

const schema77 = {"properties":{"bound":{"type":"string"},"branch":{"format":"uint","minimum":0,"type":"integer"},"bus":{"type":"string"},"load":{"type":"string"},"nominal_voltage":{"format":"double","type":"number"},"voltage":{"format":"double","type":"number"},"voltage_pu":{"format":"double","type":"number"}},"required":["load","branch","bus","voltage","nominal_voltage","voltage_pu","bound"],"type":"object"};

function validate190(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate190.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.load === undefined) && (missing0 = "load")) || ((data.branch === undefined) && (missing0 = "branch"))) || ((data.bus === undefined) && (missing0 = "bus"))) || ((data.voltage === undefined) && (missing0 = "voltage"))) || ((data.nominal_voltage === undefined) && (missing0 = "nominal_voltage"))) || ((data.voltage_pu === undefined) && (missing0 = "voltage_pu"))) || ((data.bound === undefined) && (missing0 = "bound"))){
validate190.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bound !== undefined){
const _errs1 = errors;
if(typeof data.bound !== "string"){
validate190.errors = [{instancePath:instancePath+"/bound",schemaPath:"#/properties/bound/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.branch !== undefined){
let data1 = data.branch;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate190.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate190.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.bus !== undefined){
const _errs5 = errors;
if(typeof data.bus !== "string"){
validate190.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.load !== undefined){
const _errs7 = errors;
if(typeof data.load !== "string"){
validate190.errors = [{instancePath:instancePath+"/load",schemaPath:"#/properties/load/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.nominal_voltage !== undefined){
let data4 = data.nominal_voltage;
const _errs9 = errors;
if(errors === _errs9){
if(!((typeof data4 == "number") && (isFinite(data4)))){
validate190.errors = [{instancePath:instancePath+"/nominal_voltage",schemaPath:"#/properties/nominal_voltage/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage !== undefined){
let data5 = data.voltage;
const _errs11 = errors;
if(errors === _errs11){
if(!((typeof data5 == "number") && (isFinite(data5)))){
validate190.errors = [{instancePath:instancePath+"/voltage",schemaPath:"#/properties/voltage/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_pu !== undefined){
let data6 = data.voltage_pu;
const _errs13 = errors;
if(errors === _errs13){
if(!((typeof data6 == "number") && (isFinite(data6)))){
validate190.errors = [{instancePath:instancePath+"/voltage_pu",schemaPath:"#/properties/voltage_pu/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate190.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate190.errors = vErrors;
return errors === 0;
}
validate190.evaluated = {"props":{"bound":true,"branch":true,"bus":true,"load":true,"nominal_voltage":true,"voltage":true,"voltage_pu":true},"dynamicProps":false,"dynamicItems":false};


function validate344(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate344.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((((((((data.converged === undefined) && (missing0 = "converged")) || ((data.voltage_valid === undefined) && (missing0 = "voltage_valid"))) || ((data.voltage_violations === undefined) && (missing0 = "voltage_violations"))) || ((data.iterations === undefined) && (missing0 = "iterations"))) || ((data.factorization_count === undefined) && (missing0 = "factorization_count"))) || ((data.matrix_dimension === undefined) && (missing0 = "matrix_dimension"))) || ((data.matrix_nonzeros === undefined) && (missing0 = "matrix_nonzeros"))) || ((data.voltage_change === undefined) && (missing0 = "voltage_change"))) || ((data.physical_kcl_residual === undefined) && (missing0 = "physical_kcl_residual"))) || ((data.scaled_kcl_residual === undefined) && (missing0 = "scaled_kcl_residual"))) || ((data.terminals === undefined) && (missing0 = "terminals"))) || ((data.element_ports === undefined) && (missing0 = "element_ports"))) || ((data.source_reactions === undefined) && (missing0 = "source_reactions"))){
validate344.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.converged !== undefined){
const _errs1 = errors;
if(typeof data.converged !== "boolean"){
validate344.errors = [{instancePath:instancePath+"/converged",schemaPath:"#/properties/converged/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.element_ports !== undefined){
let data1 = data.element_ports;
const _errs3 = errors;
if(errors === _errs3){
if(Array.isArray(data1)){
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
const _errs5 = errors;
if(!(validate152(data1[i0], {instancePath:instancePath+"/element_ports/" + i0,parentData:data1,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate152.errors : vErrors.concat(validate152.errors);
errors = vErrors.length;
}
var valid1 = _errs5 === errors;
if(!valid1){
break;
}
}
}
else {
validate344.errors = [{instancePath:instancePath+"/element_ports",schemaPath:"#/properties/element_ports/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.excluded_islands !== undefined){
let data3 = data.excluded_islands;
const _errs6 = errors;
if(errors === _errs6){
if(Array.isArray(data3)){
var valid2 = true;
const len1 = data3.length;
for(let i1=0; i1<len1; i1++){
const _errs8 = errors;
if(!(validate169(data3[i1], {instancePath:instancePath+"/excluded_islands/" + i1,parentData:data3,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate169.errors : vErrors.concat(validate169.errors);
errors = vErrors.length;
}
var valid2 = _errs8 === errors;
if(!valid2){
break;
}
}
}
else {
validate344.errors = [{instancePath:instancePath+"/excluded_islands",schemaPath:"#/properties/excluded_islands/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.factorization_count !== undefined){
let data5 = data.factorization_count;
const _errs9 = errors;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
validate344.errors = [{instancePath:instancePath+"/factorization_count",schemaPath:"#/properties/factorization_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs9){
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
validate344.errors = [{instancePath:instancePath+"/factorization_count",schemaPath:"#/properties/factorization_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iterations !== undefined){
let data6 = data.iterations;
const _errs11 = errors;
if(!(((typeof data6 == "number") && (!(data6 % 1) && !isNaN(data6))) && (isFinite(data6)))){
validate344.errors = [{instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs11){
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 < 0 || isNaN(data6)){
validate344.errors = [{instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.load_model !== undefined){
let data7 = data.load_model;
const _errs13 = errors;
const _errs14 = errors;
let valid3 = false;
const _errs15 = errors;
if(!(validate173(data7, {instancePath:instancePath+"/load_model",parentData:data,parentDataProperty:"load_model",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate173.errors : vErrors.concat(validate173.errors);
errors = vErrors.length;
}
var _valid0 = _errs15 === errors;
valid3 = valid3 || _valid0;
if(_valid0){
var props0 = {};
props0.modified_branch_count = true;
props0.options = true;
props0.original_kcl_residual = true;
props0.original_scaled_kcl_residual = true;
props0.status = true;
}
const _errs16 = errors;
if(data7 !== null){
const err0 = {instancePath:instancePath+"/load_model",schemaPath:"#/properties/load_model/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs16 === errors;
valid3 = valid3 || _valid0;
if(!valid3){
const err1 = {instancePath:instancePath+"/load_model",schemaPath:"#/properties/load_model/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate344.errors = vErrors;
return false;
}
else {
errors = _errs14;
if(vErrors !== null){
if(_errs14){
vErrors.length = _errs14;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.load_model_changes !== undefined){
let data8 = data.load_model_changes;
const _errs18 = errors;
if(errors === _errs18){
if(Array.isArray(data8)){
var valid4 = true;
const len2 = data8.length;
for(let i2=0; i2<len2; i2++){
const _errs20 = errors;
if(!(validate179(data8[i2], {instancePath:instancePath+"/load_model_changes/" + i2,parentData:data8,parentDataProperty:i2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate179.errors : vErrors.concat(validate179.errors);
errors = vErrors.length;
}
var valid4 = _errs20 === errors;
if(!valid4){
break;
}
}
}
else {
validate344.errors = [{instancePath:instancePath+"/load_model_changes",schemaPath:"#/properties/load_model_changes/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.matrix_dimension !== undefined){
let data10 = data.matrix_dimension;
const _errs21 = errors;
if(!(((typeof data10 == "number") && (!(data10 % 1) && !isNaN(data10))) && (isFinite(data10)))){
validate344.errors = [{instancePath:instancePath+"/matrix_dimension",schemaPath:"#/properties/matrix_dimension/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs21){
if((typeof data10 == "number") && (isFinite(data10))){
if(data10 < 0 || isNaN(data10)){
validate344.errors = [{instancePath:instancePath+"/matrix_dimension",schemaPath:"#/properties/matrix_dimension/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.matrix_nonzeros !== undefined){
let data11 = data.matrix_nonzeros;
const _errs23 = errors;
if(!(((typeof data11 == "number") && (!(data11 % 1) && !isNaN(data11))) && (isFinite(data11)))){
validate344.errors = [{instancePath:instancePath+"/matrix_nonzeros",schemaPath:"#/properties/matrix_nonzeros/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs23){
if((typeof data11 == "number") && (isFinite(data11))){
if(data11 < 0 || isNaN(data11)){
validate344.errors = [{instancePath:instancePath+"/matrix_nonzeros",schemaPath:"#/properties/matrix_nonzeros/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs23 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.max_voltage_pu !== undefined){
let data12 = data.max_voltage_pu;
const _errs25 = errors;
if((!((typeof data12 == "number") && (isFinite(data12)))) && (data12 !== null)){
validate344.errors = [{instancePath:instancePath+"/max_voltage_pu",schemaPath:"#/properties/max_voltage_pu/type",keyword:"type",params:{type: schema68.properties.max_voltage_pu.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs25 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.min_voltage_pu !== undefined){
let data13 = data.min_voltage_pu;
const _errs27 = errors;
if((!((typeof data13 == "number") && (isFinite(data13)))) && (data13 !== null)){
validate344.errors = [{instancePath:instancePath+"/min_voltage_pu",schemaPath:"#/properties/min_voltage_pu/type",keyword:"type",params:{type: schema68.properties.min_voltage_pu.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs27 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.network_scope !== undefined){
let data14 = data.network_scope;
const _errs29 = errors;
const _errs30 = errors;
let valid5 = false;
const _errs31 = errors;
if(!(validate183(data14, {instancePath:instancePath+"/network_scope",parentData:data,parentDataProperty:"network_scope",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate183.errors : vErrors.concat(validate183.errors);
errors = vErrors.length;
}
var _valid1 = _errs31 === errors;
valid5 = valid5 || _valid1;
if(_valid1){
var props1 = {};
props1.excluded_bus_count = true;
props1.excluded_equipment_count = true;
props1.excluded_island_count = true;
props1.excluded_load_count = true;
}
const _errs32 = errors;
if(data14 !== null){
const err2 = {instancePath:instancePath+"/network_scope",schemaPath:"#/properties/network_scope/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs32 === errors;
valid5 = valid5 || _valid1;
if(!valid5){
const err3 = {instancePath:instancePath+"/network_scope",schemaPath:"#/properties/network_scope/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate344.errors = vErrors;
return false;
}
else {
errors = _errs30;
if(vErrors !== null){
if(_errs30){
vErrors.length = _errs30;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs29 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.physical_kcl_residual !== undefined){
let data15 = data.physical_kcl_residual;
const _errs34 = errors;
if(errors === _errs34){
if(!((typeof data15 == "number") && (isFinite(data15)))){
validate344.errors = [{instancePath:instancePath+"/physical_kcl_residual",schemaPath:"#/properties/physical_kcl_residual/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs34 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.scaled_kcl_residual !== undefined){
let data16 = data.scaled_kcl_residual;
const _errs36 = errors;
if(errors === _errs36){
if(!((typeof data16 == "number") && (isFinite(data16)))){
validate344.errors = [{instancePath:instancePath+"/scaled_kcl_residual",schemaPath:"#/properties/scaled_kcl_residual/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs36 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.source_reactions !== undefined){
let data17 = data.source_reactions;
const _errs38 = errors;
if(errors === _errs38){
if(Array.isArray(data17)){
var valid6 = true;
const len3 = data17.length;
for(let i3=0; i3<len3; i3++){
const _errs40 = errors;
if(!(validate185(data17[i3], {instancePath:instancePath+"/source_reactions/" + i3,parentData:data17,parentDataProperty:i3,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate185.errors : vErrors.concat(validate185.errors);
errors = vErrors.length;
}
var valid6 = _errs40 === errors;
if(!valid6){
break;
}
}
}
else {
validate344.errors = [{instancePath:instancePath+"/source_reactions",schemaPath:"#/properties/source_reactions/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs38 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.terminals !== undefined){
let data19 = data.terminals;
const _errs41 = errors;
if(errors === _errs41){
if(Array.isArray(data19)){
var valid7 = true;
const len4 = data19.length;
for(let i4=0; i4<len4; i4++){
const _errs43 = errors;
if(!(validate157(data19[i4], {instancePath:instancePath+"/terminals/" + i4,parentData:data19,parentDataProperty:i4,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate157.errors : vErrors.concat(validate157.errors);
errors = vErrors.length;
}
var valid7 = _errs43 === errors;
if(!valid7){
break;
}
}
}
else {
validate344.errors = [{instancePath:instancePath+"/terminals",schemaPath:"#/properties/terminals/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs41 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_change !== undefined){
let data21 = data.voltage_change;
const _errs44 = errors;
if(errors === _errs44){
if(!((typeof data21 == "number") && (isFinite(data21)))){
validate344.errors = [{instancePath:instancePath+"/voltage_change",schemaPath:"#/properties/voltage_change/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs44 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_valid !== undefined){
const _errs46 = errors;
if(typeof data.voltage_valid !== "boolean"){
validate344.errors = [{instancePath:instancePath+"/voltage_valid",schemaPath:"#/properties/voltage_valid/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs46 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_violations !== undefined){
let data23 = data.voltage_violations;
const _errs48 = errors;
if(errors === _errs48){
if(Array.isArray(data23)){
var valid8 = true;
const len5 = data23.length;
for(let i5=0; i5<len5; i5++){
const _errs50 = errors;
if(!(validate190(data23[i5], {instancePath:instancePath+"/voltage_violations/" + i5,parentData:data23,parentDataProperty:i5,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate190.errors : vErrors.concat(validate190.errors);
errors = vErrors.length;
}
var valid8 = _errs50 === errors;
if(!valid8){
break;
}
}
}
else {
validate344.errors = [{instancePath:instancePath+"/voltage_violations",schemaPath:"#/properties/voltage_violations/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs48 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate344.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate344.errors = vErrors;
return errors === 0;
}
validate344.evaluated = {"props":{"converged":true,"element_ports":true,"excluded_islands":true,"factorization_count":true,"iterations":true,"load_model":true,"load_model_changes":true,"matrix_dimension":true,"matrix_nonzeros":true,"max_voltage_pu":true,"min_voltage_pu":true,"network_scope":true,"physical_kcl_residual":true,"scaled_kcl_residual":true,"source_reactions":true,"terminals":true,"voltage_change":true,"voltage_valid":true,"voltage_violations":true},"dynamicProps":false,"dynamicItems":false};

export const mc_study = validate353;
const schema78 = {"description":"A portable, replayable distribution Study snapshot.\n\nThis deliberately sits beside the balanced KKT Study rather than pretending\nthat a multiconductor fixed-point result has balanced buses, objectives, or\nsensitivity columns. The input, effective options and rich terminal/element\nresult make it self-contained. Only a verified original-model result also\ncarries a canonical PowerIO solution module.","properties":{"formulation":{"type":"string"},"id":{"type":"string"},"input_module":{"type":"string"},"options":{"$ref":"#/$defs/McPfOptions"},"result":{"$ref":"#/$defs/McPfResult"},"schema":{"type":"string"},"solution_module":{"description":"None for modified or unverified load laws; result remains fully inspectable.","type":["string","null"]},"title":{"type":"string"},"version":{"format":"uint32","minimum":0,"type":"integer"}},"required":["schema","version","id","title","formulation","input_module","options","result"],"type":"object"};

function validate167(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate167.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((((((((data.converged === undefined) && (missing0 = "converged")) || ((data.voltage_valid === undefined) && (missing0 = "voltage_valid"))) || ((data.voltage_violations === undefined) && (missing0 = "voltage_violations"))) || ((data.iterations === undefined) && (missing0 = "iterations"))) || ((data.factorization_count === undefined) && (missing0 = "factorization_count"))) || ((data.matrix_dimension === undefined) && (missing0 = "matrix_dimension"))) || ((data.matrix_nonzeros === undefined) && (missing0 = "matrix_nonzeros"))) || ((data.voltage_change === undefined) && (missing0 = "voltage_change"))) || ((data.physical_kcl_residual === undefined) && (missing0 = "physical_kcl_residual"))) || ((data.scaled_kcl_residual === undefined) && (missing0 = "scaled_kcl_residual"))) || ((data.terminals === undefined) && (missing0 = "terminals"))) || ((data.element_ports === undefined) && (missing0 = "element_ports"))) || ((data.source_reactions === undefined) && (missing0 = "source_reactions"))){
validate167.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.converged !== undefined){
const _errs1 = errors;
if(typeof data.converged !== "boolean"){
validate167.errors = [{instancePath:instancePath+"/converged",schemaPath:"#/properties/converged/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.element_ports !== undefined){
let data1 = data.element_ports;
const _errs3 = errors;
if(errors === _errs3){
if(Array.isArray(data1)){
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
const _errs5 = errors;
if(!(validate152(data1[i0], {instancePath:instancePath+"/element_ports/" + i0,parentData:data1,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate152.errors : vErrors.concat(validate152.errors);
errors = vErrors.length;
}
var valid1 = _errs5 === errors;
if(!valid1){
break;
}
}
}
else {
validate167.errors = [{instancePath:instancePath+"/element_ports",schemaPath:"#/properties/element_ports/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.excluded_islands !== undefined){
let data3 = data.excluded_islands;
const _errs6 = errors;
if(errors === _errs6){
if(Array.isArray(data3)){
var valid2 = true;
const len1 = data3.length;
for(let i1=0; i1<len1; i1++){
const _errs8 = errors;
if(!(validate169(data3[i1], {instancePath:instancePath+"/excluded_islands/" + i1,parentData:data3,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate169.errors : vErrors.concat(validate169.errors);
errors = vErrors.length;
}
var valid2 = _errs8 === errors;
if(!valid2){
break;
}
}
}
else {
validate167.errors = [{instancePath:instancePath+"/excluded_islands",schemaPath:"#/properties/excluded_islands/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.factorization_count !== undefined){
let data5 = data.factorization_count;
const _errs9 = errors;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
validate167.errors = [{instancePath:instancePath+"/factorization_count",schemaPath:"#/properties/factorization_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs9){
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
validate167.errors = [{instancePath:instancePath+"/factorization_count",schemaPath:"#/properties/factorization_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iterations !== undefined){
let data6 = data.iterations;
const _errs11 = errors;
if(!(((typeof data6 == "number") && (!(data6 % 1) && !isNaN(data6))) && (isFinite(data6)))){
validate167.errors = [{instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs11){
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 < 0 || isNaN(data6)){
validate167.errors = [{instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.load_model !== undefined){
let data7 = data.load_model;
const _errs13 = errors;
const _errs14 = errors;
let valid3 = false;
const _errs15 = errors;
if(!(validate173(data7, {instancePath:instancePath+"/load_model",parentData:data,parentDataProperty:"load_model",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate173.errors : vErrors.concat(validate173.errors);
errors = vErrors.length;
}
var _valid0 = _errs15 === errors;
valid3 = valid3 || _valid0;
if(_valid0){
var props0 = {};
props0.modified_branch_count = true;
props0.options = true;
props0.original_kcl_residual = true;
props0.original_scaled_kcl_residual = true;
props0.status = true;
}
const _errs16 = errors;
if(data7 !== null){
const err0 = {instancePath:instancePath+"/load_model",schemaPath:"#/properties/load_model/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs16 === errors;
valid3 = valid3 || _valid0;
if(!valid3){
const err1 = {instancePath:instancePath+"/load_model",schemaPath:"#/properties/load_model/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate167.errors = vErrors;
return false;
}
else {
errors = _errs14;
if(vErrors !== null){
if(_errs14){
vErrors.length = _errs14;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.load_model_changes !== undefined){
let data8 = data.load_model_changes;
const _errs18 = errors;
if(errors === _errs18){
if(Array.isArray(data8)){
var valid4 = true;
const len2 = data8.length;
for(let i2=0; i2<len2; i2++){
const _errs20 = errors;
if(!(validate179(data8[i2], {instancePath:instancePath+"/load_model_changes/" + i2,parentData:data8,parentDataProperty:i2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate179.errors : vErrors.concat(validate179.errors);
errors = vErrors.length;
}
var valid4 = _errs20 === errors;
if(!valid4){
break;
}
}
}
else {
validate167.errors = [{instancePath:instancePath+"/load_model_changes",schemaPath:"#/properties/load_model_changes/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.matrix_dimension !== undefined){
let data10 = data.matrix_dimension;
const _errs21 = errors;
if(!(((typeof data10 == "number") && (!(data10 % 1) && !isNaN(data10))) && (isFinite(data10)))){
validate167.errors = [{instancePath:instancePath+"/matrix_dimension",schemaPath:"#/properties/matrix_dimension/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs21){
if((typeof data10 == "number") && (isFinite(data10))){
if(data10 < 0 || isNaN(data10)){
validate167.errors = [{instancePath:instancePath+"/matrix_dimension",schemaPath:"#/properties/matrix_dimension/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.matrix_nonzeros !== undefined){
let data11 = data.matrix_nonzeros;
const _errs23 = errors;
if(!(((typeof data11 == "number") && (!(data11 % 1) && !isNaN(data11))) && (isFinite(data11)))){
validate167.errors = [{instancePath:instancePath+"/matrix_nonzeros",schemaPath:"#/properties/matrix_nonzeros/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs23){
if((typeof data11 == "number") && (isFinite(data11))){
if(data11 < 0 || isNaN(data11)){
validate167.errors = [{instancePath:instancePath+"/matrix_nonzeros",schemaPath:"#/properties/matrix_nonzeros/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs23 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.max_voltage_pu !== undefined){
let data12 = data.max_voltage_pu;
const _errs25 = errors;
if((!((typeof data12 == "number") && (isFinite(data12)))) && (data12 !== null)){
validate167.errors = [{instancePath:instancePath+"/max_voltage_pu",schemaPath:"#/properties/max_voltage_pu/type",keyword:"type",params:{type: schema68.properties.max_voltage_pu.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs25 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.min_voltage_pu !== undefined){
let data13 = data.min_voltage_pu;
const _errs27 = errors;
if((!((typeof data13 == "number") && (isFinite(data13)))) && (data13 !== null)){
validate167.errors = [{instancePath:instancePath+"/min_voltage_pu",schemaPath:"#/properties/min_voltage_pu/type",keyword:"type",params:{type: schema68.properties.min_voltage_pu.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs27 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.network_scope !== undefined){
let data14 = data.network_scope;
const _errs29 = errors;
const _errs30 = errors;
let valid5 = false;
const _errs31 = errors;
if(!(validate183(data14, {instancePath:instancePath+"/network_scope",parentData:data,parentDataProperty:"network_scope",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate183.errors : vErrors.concat(validate183.errors);
errors = vErrors.length;
}
var _valid1 = _errs31 === errors;
valid5 = valid5 || _valid1;
if(_valid1){
var props1 = {};
props1.excluded_bus_count = true;
props1.excluded_equipment_count = true;
props1.excluded_island_count = true;
props1.excluded_load_count = true;
}
const _errs32 = errors;
if(data14 !== null){
const err2 = {instancePath:instancePath+"/network_scope",schemaPath:"#/properties/network_scope/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs32 === errors;
valid5 = valid5 || _valid1;
if(!valid5){
const err3 = {instancePath:instancePath+"/network_scope",schemaPath:"#/properties/network_scope/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate167.errors = vErrors;
return false;
}
else {
errors = _errs30;
if(vErrors !== null){
if(_errs30){
vErrors.length = _errs30;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs29 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.physical_kcl_residual !== undefined){
let data15 = data.physical_kcl_residual;
const _errs34 = errors;
if(errors === _errs34){
if(!((typeof data15 == "number") && (isFinite(data15)))){
validate167.errors = [{instancePath:instancePath+"/physical_kcl_residual",schemaPath:"#/properties/physical_kcl_residual/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs34 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.scaled_kcl_residual !== undefined){
let data16 = data.scaled_kcl_residual;
const _errs36 = errors;
if(errors === _errs36){
if(!((typeof data16 == "number") && (isFinite(data16)))){
validate167.errors = [{instancePath:instancePath+"/scaled_kcl_residual",schemaPath:"#/properties/scaled_kcl_residual/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs36 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.source_reactions !== undefined){
let data17 = data.source_reactions;
const _errs38 = errors;
if(errors === _errs38){
if(Array.isArray(data17)){
var valid6 = true;
const len3 = data17.length;
for(let i3=0; i3<len3; i3++){
const _errs40 = errors;
if(!(validate185(data17[i3], {instancePath:instancePath+"/source_reactions/" + i3,parentData:data17,parentDataProperty:i3,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate185.errors : vErrors.concat(validate185.errors);
errors = vErrors.length;
}
var valid6 = _errs40 === errors;
if(!valid6){
break;
}
}
}
else {
validate167.errors = [{instancePath:instancePath+"/source_reactions",schemaPath:"#/properties/source_reactions/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs38 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.terminals !== undefined){
let data19 = data.terminals;
const _errs41 = errors;
if(errors === _errs41){
if(Array.isArray(data19)){
var valid7 = true;
const len4 = data19.length;
for(let i4=0; i4<len4; i4++){
const _errs43 = errors;
if(!(validate157(data19[i4], {instancePath:instancePath+"/terminals/" + i4,parentData:data19,parentDataProperty:i4,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate157.errors : vErrors.concat(validate157.errors);
errors = vErrors.length;
}
var valid7 = _errs43 === errors;
if(!valid7){
break;
}
}
}
else {
validate167.errors = [{instancePath:instancePath+"/terminals",schemaPath:"#/properties/terminals/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs41 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_change !== undefined){
let data21 = data.voltage_change;
const _errs44 = errors;
if(errors === _errs44){
if(!((typeof data21 == "number") && (isFinite(data21)))){
validate167.errors = [{instancePath:instancePath+"/voltage_change",schemaPath:"#/properties/voltage_change/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs44 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_valid !== undefined){
const _errs46 = errors;
if(typeof data.voltage_valid !== "boolean"){
validate167.errors = [{instancePath:instancePath+"/voltage_valid",schemaPath:"#/properties/voltage_valid/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs46 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_violations !== undefined){
let data23 = data.voltage_violations;
const _errs48 = errors;
if(errors === _errs48){
if(Array.isArray(data23)){
var valid8 = true;
const len5 = data23.length;
for(let i5=0; i5<len5; i5++){
const _errs50 = errors;
if(!(validate190(data23[i5], {instancePath:instancePath+"/voltage_violations/" + i5,parentData:data23,parentDataProperty:i5,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate190.errors : vErrors.concat(validate190.errors);
errors = vErrors.length;
}
var valid8 = _errs50 === errors;
if(!valid8){
break;
}
}
}
else {
validate167.errors = [{instancePath:instancePath+"/voltage_violations",schemaPath:"#/properties/voltage_violations/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs48 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate167.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate167.errors = vErrors;
return errors === 0;
}
validate167.evaluated = {"props":{"converged":true,"element_ports":true,"excluded_islands":true,"factorization_count":true,"iterations":true,"load_model":true,"load_model_changes":true,"matrix_dimension":true,"matrix_nonzeros":true,"max_voltage_pu":true,"min_voltage_pu":true,"network_scope":true,"physical_kcl_residual":true,"scaled_kcl_residual":true,"source_reactions":true,"terminals":true,"voltage_change":true,"voltage_valid":true,"voltage_violations":true},"dynamicProps":false,"dynamicItems":false};


function validate353(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate353.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((((data.schema === undefined) && (missing0 = "schema")) || ((data.version === undefined) && (missing0 = "version"))) || ((data.id === undefined) && (missing0 = "id"))) || ((data.title === undefined) && (missing0 = "title"))) || ((data.formulation === undefined) && (missing0 = "formulation"))) || ((data.input_module === undefined) && (missing0 = "input_module"))) || ((data.options === undefined) && (missing0 = "options"))) || ((data.result === undefined) && (missing0 = "result"))){
validate353.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.formulation !== undefined){
const _errs1 = errors;
if(typeof data.formulation !== "string"){
validate353.errors = [{instancePath:instancePath+"/formulation",schemaPath:"#/properties/formulation/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.id !== undefined){
const _errs3 = errors;
if(typeof data.id !== "string"){
validate353.errors = [{instancePath:instancePath+"/id",schemaPath:"#/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.input_module !== undefined){
const _errs5 = errors;
if(typeof data.input_module !== "string"){
validate353.errors = [{instancePath:instancePath+"/input_module",schemaPath:"#/properties/input_module/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.options !== undefined){
const _errs7 = errors;
if(!(validate174(data.options, {instancePath:instancePath+"/options",parentData:data,parentDataProperty:"options",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate174.errors : vErrors.concat(validate174.errors);
errors = vErrors.length;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.result !== undefined){
const _errs8 = errors;
if(!(validate167(data.result, {instancePath:instancePath+"/result",parentData:data,parentDataProperty:"result",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate167.errors : vErrors.concat(validate167.errors);
errors = vErrors.length;
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.schema !== undefined){
const _errs9 = errors;
if(typeof data.schema !== "string"){
validate353.errors = [{instancePath:instancePath+"/schema",schemaPath:"#/properties/schema/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.solution_module !== undefined){
let data6 = data.solution_module;
const _errs11 = errors;
if((typeof data6 !== "string") && (data6 !== null)){
validate353.errors = [{instancePath:instancePath+"/solution_module",schemaPath:"#/properties/solution_module/type",keyword:"type",params:{type: schema78.properties.solution_module.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.title !== undefined){
const _errs13 = errors;
if(typeof data.title !== "string"){
validate353.errors = [{instancePath:instancePath+"/title",schemaPath:"#/properties/title/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.version !== undefined){
let data8 = data.version;
const _errs15 = errors;
if(!(((typeof data8 == "number") && (!(data8 % 1) && !isNaN(data8))) && (isFinite(data8)))){
validate353.errors = [{instancePath:instancePath+"/version",schemaPath:"#/properties/version/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs15){
if((typeof data8 == "number") && (isFinite(data8))){
if(data8 < 0 || isNaN(data8)){
validate353.errors = [{instancePath:instancePath+"/version",schemaPath:"#/properties/version/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
else {
validate353.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate353.errors = vErrors;
return errors === 0;
}
validate353.evaluated = {"props":{"formulation":true,"id":true,"input_module":true,"options":true,"result":true,"schema":true,"solution_module":true,"title":true,"version":true},"dynamicProps":false,"dynamicItems":false};

export const mc_summary = validate356;
const schema79 = {"description":"The compact, constant-size view of one converged operating point.\n\nScalar fields carry the same names and values as [`McPfResult`]; the\naggregates are the sums a results summary displays. It is computed from\nthe converged phasors without building per-terminal or per-port records,\nso its cost and size do not depend on how the network is displayed.","properties":{"converged":{"type":"boolean"},"element_port_count":{"format":"uint","minimum":0,"type":"integer"},"factorization_count":{"format":"uint","minimum":0,"type":"integer"},"iterations":{"format":"uint","minimum":0,"type":"integer"},"load_model":{"anyOf":[{"$ref":"#/$defs/McLoadModelAssessment"},{"type":"null"}],"default":null},"matrix_dimension":{"format":"uint","minimum":0,"type":"integer"},"matrix_nonzeros":{"format":"uint","minimum":0,"type":"integer"},"max_voltage_pu":{"format":"double","type":["number","null"]},"min_voltage_pu":{"format":"double","type":["number","null"]},"network_scope":{"anyOf":[{"$ref":"#/$defs/McNetworkScope"},{"type":"null"}]},"passive_loss":{"$ref":"#/$defs/McComplex","description":"Sum of `V * conj(I_into_element)` over the ports of passive equipment\n(lines, shunts, capacitors, transformers), VA."},"physical_kcl_residual":{"format":"double","type":"number"},"scaled_kcl_residual":{"format":"double","type":"number"},"solve_count":{"description":"Solves performed by the producing session; detail views echo it so a\nreader can discard values from an older operating point.","format":"uint","minimum":0,"type":"integer"},"source_power_into_network":{"$ref":"#/$defs/McComplex","description":"Sum of every source reaction's `V * conj(I_into_network)`, VA."},"terminal_count":{"format":"uint","minimum":0,"type":"integer"},"voltage_change":{"format":"double","type":"number"},"voltage_valid":{"description":"Whether every load branch lies inside the configured normal voltage band.","type":"boolean"},"voltage_violation_count":{"description":"Number of load branches outside the normal voltage band.","format":"uint","minimum":0,"type":"integer"}},"required":["converged","voltage_valid","voltage_violation_count","iterations","factorization_count","matrix_dimension","matrix_nonzeros","voltage_change","physical_kcl_residual","scaled_kcl_residual","terminal_count","element_port_count","source_power_into_network","passive_loss","solve_count"],"type":"object"};

function validate356(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate356.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((((((((((data.converged === undefined) && (missing0 = "converged")) || ((data.voltage_valid === undefined) && (missing0 = "voltage_valid"))) || ((data.voltage_violation_count === undefined) && (missing0 = "voltage_violation_count"))) || ((data.iterations === undefined) && (missing0 = "iterations"))) || ((data.factorization_count === undefined) && (missing0 = "factorization_count"))) || ((data.matrix_dimension === undefined) && (missing0 = "matrix_dimension"))) || ((data.matrix_nonzeros === undefined) && (missing0 = "matrix_nonzeros"))) || ((data.voltage_change === undefined) && (missing0 = "voltage_change"))) || ((data.physical_kcl_residual === undefined) && (missing0 = "physical_kcl_residual"))) || ((data.scaled_kcl_residual === undefined) && (missing0 = "scaled_kcl_residual"))) || ((data.terminal_count === undefined) && (missing0 = "terminal_count"))) || ((data.element_port_count === undefined) && (missing0 = "element_port_count"))) || ((data.source_power_into_network === undefined) && (missing0 = "source_power_into_network"))) || ((data.passive_loss === undefined) && (missing0 = "passive_loss"))) || ((data.solve_count === undefined) && (missing0 = "solve_count"))){
validate356.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.converged !== undefined){
const _errs1 = errors;
if(typeof data.converged !== "boolean"){
validate356.errors = [{instancePath:instancePath+"/converged",schemaPath:"#/properties/converged/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.element_port_count !== undefined){
let data1 = data.element_port_count;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate356.errors = [{instancePath:instancePath+"/element_port_count",schemaPath:"#/properties/element_port_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate356.errors = [{instancePath:instancePath+"/element_port_count",schemaPath:"#/properties/element_port_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.factorization_count !== undefined){
let data2 = data.factorization_count;
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate356.errors = [{instancePath:instancePath+"/factorization_count",schemaPath:"#/properties/factorization_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate356.errors = [{instancePath:instancePath+"/factorization_count",schemaPath:"#/properties/factorization_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iterations !== undefined){
let data3 = data.iterations;
const _errs7 = errors;
if(!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))){
validate356.errors = [{instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs7){
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 0 || isNaN(data3)){
validate356.errors = [{instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.load_model !== undefined){
let data4 = data.load_model;
const _errs9 = errors;
const _errs10 = errors;
let valid1 = false;
const _errs11 = errors;
if(!(validate173(data4, {instancePath:instancePath+"/load_model",parentData:data,parentDataProperty:"load_model",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate173.errors : vErrors.concat(validate173.errors);
errors = vErrors.length;
}
var _valid0 = _errs11 === errors;
valid1 = valid1 || _valid0;
if(_valid0){
var props0 = {};
props0.modified_branch_count = true;
props0.options = true;
props0.original_kcl_residual = true;
props0.original_scaled_kcl_residual = true;
props0.status = true;
}
const _errs12 = errors;
if(data4 !== null){
const err0 = {instancePath:instancePath+"/load_model",schemaPath:"#/properties/load_model/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs12 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/load_model",schemaPath:"#/properties/load_model/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate356.errors = vErrors;
return false;
}
else {
errors = _errs10;
if(vErrors !== null){
if(_errs10){
vErrors.length = _errs10;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.matrix_dimension !== undefined){
let data5 = data.matrix_dimension;
const _errs14 = errors;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
validate356.errors = [{instancePath:instancePath+"/matrix_dimension",schemaPath:"#/properties/matrix_dimension/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs14){
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
validate356.errors = [{instancePath:instancePath+"/matrix_dimension",schemaPath:"#/properties/matrix_dimension/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs14 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.matrix_nonzeros !== undefined){
let data6 = data.matrix_nonzeros;
const _errs16 = errors;
if(!(((typeof data6 == "number") && (!(data6 % 1) && !isNaN(data6))) && (isFinite(data6)))){
validate356.errors = [{instancePath:instancePath+"/matrix_nonzeros",schemaPath:"#/properties/matrix_nonzeros/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs16){
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 < 0 || isNaN(data6)){
validate356.errors = [{instancePath:instancePath+"/matrix_nonzeros",schemaPath:"#/properties/matrix_nonzeros/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs16 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.max_voltage_pu !== undefined){
let data7 = data.max_voltage_pu;
const _errs18 = errors;
if((!((typeof data7 == "number") && (isFinite(data7)))) && (data7 !== null)){
validate356.errors = [{instancePath:instancePath+"/max_voltage_pu",schemaPath:"#/properties/max_voltage_pu/type",keyword:"type",params:{type: schema79.properties.max_voltage_pu.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.min_voltage_pu !== undefined){
let data8 = data.min_voltage_pu;
const _errs20 = errors;
if((!((typeof data8 == "number") && (isFinite(data8)))) && (data8 !== null)){
validate356.errors = [{instancePath:instancePath+"/min_voltage_pu",schemaPath:"#/properties/min_voltage_pu/type",keyword:"type",params:{type: schema79.properties.min_voltage_pu.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs20 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.network_scope !== undefined){
let data9 = data.network_scope;
const _errs22 = errors;
const _errs23 = errors;
let valid2 = false;
const _errs24 = errors;
if(!(validate183(data9, {instancePath:instancePath+"/network_scope",parentData:data,parentDataProperty:"network_scope",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate183.errors : vErrors.concat(validate183.errors);
errors = vErrors.length;
}
var _valid1 = _errs24 === errors;
valid2 = valid2 || _valid1;
if(_valid1){
var props1 = {};
props1.excluded_bus_count = true;
props1.excluded_equipment_count = true;
props1.excluded_island_count = true;
props1.excluded_load_count = true;
}
const _errs25 = errors;
if(data9 !== null){
const err2 = {instancePath:instancePath+"/network_scope",schemaPath:"#/properties/network_scope/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs25 === errors;
valid2 = valid2 || _valid1;
if(!valid2){
const err3 = {instancePath:instancePath+"/network_scope",schemaPath:"#/properties/network_scope/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate356.errors = vErrors;
return false;
}
else {
errors = _errs23;
if(vErrors !== null){
if(_errs23){
vErrors.length = _errs23;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs22 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.passive_loss !== undefined){
const _errs27 = errors;
if(!(validate153(data.passive_loss, {instancePath:instancePath+"/passive_loss",parentData:data,parentDataProperty:"passive_loss",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var valid0 = _errs27 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.physical_kcl_residual !== undefined){
let data11 = data.physical_kcl_residual;
const _errs28 = errors;
if(errors === _errs28){
if(!((typeof data11 == "number") && (isFinite(data11)))){
validate356.errors = [{instancePath:instancePath+"/physical_kcl_residual",schemaPath:"#/properties/physical_kcl_residual/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs28 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.scaled_kcl_residual !== undefined){
let data12 = data.scaled_kcl_residual;
const _errs30 = errors;
if(errors === _errs30){
if(!((typeof data12 == "number") && (isFinite(data12)))){
validate356.errors = [{instancePath:instancePath+"/scaled_kcl_residual",schemaPath:"#/properties/scaled_kcl_residual/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs30 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.solve_count !== undefined){
let data13 = data.solve_count;
const _errs32 = errors;
if(!(((typeof data13 == "number") && (!(data13 % 1) && !isNaN(data13))) && (isFinite(data13)))){
validate356.errors = [{instancePath:instancePath+"/solve_count",schemaPath:"#/properties/solve_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs32){
if((typeof data13 == "number") && (isFinite(data13))){
if(data13 < 0 || isNaN(data13)){
validate356.errors = [{instancePath:instancePath+"/solve_count",schemaPath:"#/properties/solve_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs32 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.source_power_into_network !== undefined){
const _errs34 = errors;
if(!(validate153(data.source_power_into_network, {instancePath:instancePath+"/source_power_into_network",parentData:data,parentDataProperty:"source_power_into_network",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate153.errors : vErrors.concat(validate153.errors);
errors = vErrors.length;
}
var valid0 = _errs34 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.terminal_count !== undefined){
let data15 = data.terminal_count;
const _errs35 = errors;
if(!(((typeof data15 == "number") && (!(data15 % 1) && !isNaN(data15))) && (isFinite(data15)))){
validate356.errors = [{instancePath:instancePath+"/terminal_count",schemaPath:"#/properties/terminal_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs35){
if((typeof data15 == "number") && (isFinite(data15))){
if(data15 < 0 || isNaN(data15)){
validate356.errors = [{instancePath:instancePath+"/terminal_count",schemaPath:"#/properties/terminal_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs35 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_change !== undefined){
let data16 = data.voltage_change;
const _errs37 = errors;
if(errors === _errs37){
if(!((typeof data16 == "number") && (isFinite(data16)))){
validate356.errors = [{instancePath:instancePath+"/voltage_change",schemaPath:"#/properties/voltage_change/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs37 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_valid !== undefined){
const _errs39 = errors;
if(typeof data.voltage_valid !== "boolean"){
validate356.errors = [{instancePath:instancePath+"/voltage_valid",schemaPath:"#/properties/voltage_valid/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs39 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_violation_count !== undefined){
let data18 = data.voltage_violation_count;
const _errs41 = errors;
if(!(((typeof data18 == "number") && (!(data18 % 1) && !isNaN(data18))) && (isFinite(data18)))){
validate356.errors = [{instancePath:instancePath+"/voltage_violation_count",schemaPath:"#/properties/voltage_violation_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs41){
if((typeof data18 == "number") && (isFinite(data18))){
if(data18 < 0 || isNaN(data18)){
validate356.errors = [{instancePath:instancePath+"/voltage_violation_count",schemaPath:"#/properties/voltage_violation_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs41 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate356.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate356.errors = vErrors;
return errors === 0;
}
validate356.evaluated = {"props":{"converged":true,"element_port_count":true,"factorization_count":true,"iterations":true,"load_model":true,"matrix_dimension":true,"matrix_nonzeros":true,"max_voltage_pu":true,"min_voltage_pu":true,"network_scope":true,"passive_loss":true,"physical_kcl_residual":true,"scaled_kcl_residual":true,"solve_count":true,"source_power_into_network":true,"terminal_count":true,"voltage_change":true,"voltage_valid":true,"voltage_violation_count":true},"dynamicProps":false,"dynamicItems":false};

export const plan = validate361;
const schema80 = {"description":"The outcome of one bounded planning search and its unapplied\nproposal.","properties":{"baseline":{"$ref":"#/$defs/CapacityPlanResultSummary","description":"Summary of the baseline and the final accepted exact solve. The solve\nnumbers refer to this planning operation's exact solve budget."},"baseline_phi":{"description":"`Phi` at the starting exact solve.","format":"double","type":"number"},"exact_solves":{"description":"Exact OPF solves consumed, including the baseline.","format":"uint","minimum":0,"type":"integer"},"exact_verified_result":{"$ref":"#/$defs/CapacityPlanResultSummary"},"final_phi":{"description":"`Phi` at the final accepted exact solve.","format":"double","type":"number"},"iterations":{"description":"Every trial in order, accepted and rejected.","items":{"$ref":"#/$defs/CapacityPlanIteration"},"type":"array"},"proposal":{"description":"The accumulated accepted rating changes: the unapplied proposal.","items":{"$ref":"#/$defs/RatingChange"},"type":"array"},"spent_budget_mw":{"description":"Total accepted movement `Σ |delta|`, MW, against the budget.","format":"double","type":"number"}},"required":["baseline","exact_verified_result","baseline_phi","final_phi","proposal","spent_budget_mw","iterations","exact_solves"],"type":"object"};
const schema81 = {"description":"One exact result retained in the serializable planning record.","properties":{"declared_objective":{"format":"double","type":"number"},"exact_solve":{"format":"uint","minimum":0,"type":"integer"},"phi":{"format":"double","type":"number"}},"required":["phi","declared_objective","exact_solve"],"type":"object"};

function validate204(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate204.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.phi === undefined) && (missing0 = "phi")) || ((data.declared_objective === undefined) && (missing0 = "declared_objective"))) || ((data.exact_solve === undefined) && (missing0 = "exact_solve"))){
validate204.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.declared_objective !== undefined){
let data0 = data.declared_objective;
const _errs1 = errors;
if(errors === _errs1){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate204.errors = [{instancePath:instancePath+"/declared_objective",schemaPath:"#/properties/declared_objective/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.exact_solve !== undefined){
let data1 = data.exact_solve;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate204.errors = [{instancePath:instancePath+"/exact_solve",schemaPath:"#/properties/exact_solve/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate204.errors = [{instancePath:instancePath+"/exact_solve",schemaPath:"#/properties/exact_solve/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.phi !== undefined){
let data2 = data.phi;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate204.errors = [{instancePath:instancePath+"/phi",schemaPath:"#/properties/phi/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate204.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate204.errors = vErrors;
return errors === 0;
}
validate204.evaluated = {"props":{"declared_objective":true,"exact_solve":true,"phi":true},"dynamicProps":false,"dynamicItems":false};

const schema82 = {"description":"One recorded search step: the direction the gradient named, the rating\ntrial, the first order prediction, the exact outcome, and\nwhether the step was kept.","properties":{"accepted":{"type":"boolean"},"delta_mw":{"description":"The trial rating changes, MW.","items":{"$ref":"#/$defs/RatingChange"},"type":"array"},"exact_phi_delta":{"description":"Exact `Phi(after) - Phi(before)` from the trial re-solve;\n`None` when the trial solve failed.","format":"double","type":["number","null"]},"first_order_error":{"description":"`|exact - predicted|` and its ratio to `|exact|`, when the trial\nsolved.","format":"double","type":["number","null"]},"first_order_error_rel":{"format":"double","type":["number","null"]},"gradient":{"description":"`dPhi/dc` at the step's starting point for every candidate, in outer\nobjective units per MW.","items":{"$ref":"#/$defs/GradientEntry"},"type":"array"},"predicted_phi_delta":{"description":"First order prediction `Σ g_e · delta_e`.","format":"double","type":"number"},"reason":{"description":"Why the step was kept or dropped, one short sentence.","type":"string"}},"required":["gradient","delta_mw","predicted_phi_delta","accepted","reason"],"type":"object"};
const schema83 = {"description":"One branch rating change of a proposal, MW, by canonical PowerIO identity.","properties":{"branch":{"type":"string"},"delta_mw":{"format":"double","type":"number"}},"required":["branch","delta_mw"],"type":"object"};

function validate208(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate208.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.branch === undefined) && (missing0 = "branch")) || ((data.delta_mw === undefined) && (missing0 = "delta_mw"))){
validate208.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch !== undefined){
const _errs1 = errors;
if(typeof data.branch !== "string"){
validate208.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.delta_mw !== undefined){
let data1 = data.delta_mw;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate208.errors = [{instancePath:instancePath+"/delta_mw",schemaPath:"#/properties/delta_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate208.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate208.errors = vErrors;
return errors === 0;
}
validate208.evaluated = {"props":{"branch":true,"delta_mw":true},"dynamicProps":false,"dynamicItems":false};

const schema84 = {"description":"One implicit gradient entry, in outer objective units per MW, by canonical\nPowerIO identity.","properties":{"branch":{"type":"string"},"value":{"format":"double","type":"number"}},"required":["branch","value"],"type":"object"};

function validate210(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate210.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.branch === undefined) && (missing0 = "branch")) || ((data.value === undefined) && (missing0 = "value"))){
validate210.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch !== undefined){
const _errs1 = errors;
if(typeof data.branch !== "string"){
validate210.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data1 = data.value;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate210.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate210.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate210.errors = vErrors;
return errors === 0;
}
validate210.evaluated = {"props":{"branch":true,"value":true},"dynamicProps":false,"dynamicItems":false};


function validate207(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate207.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((data.gradient === undefined) && (missing0 = "gradient")) || ((data.delta_mw === undefined) && (missing0 = "delta_mw"))) || ((data.predicted_phi_delta === undefined) && (missing0 = "predicted_phi_delta"))) || ((data.accepted === undefined) && (missing0 = "accepted"))) || ((data.reason === undefined) && (missing0 = "reason"))){
validate207.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.accepted !== undefined){
const _errs1 = errors;
if(typeof data.accepted !== "boolean"){
validate207.errors = [{instancePath:instancePath+"/accepted",schemaPath:"#/properties/accepted/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.delta_mw !== undefined){
let data1 = data.delta_mw;
const _errs3 = errors;
if(errors === _errs3){
if(Array.isArray(data1)){
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
const _errs5 = errors;
if(!(validate208(data1[i0], {instancePath:instancePath+"/delta_mw/" + i0,parentData:data1,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate208.errors : vErrors.concat(validate208.errors);
errors = vErrors.length;
}
var valid1 = _errs5 === errors;
if(!valid1){
break;
}
}
}
else {
validate207.errors = [{instancePath:instancePath+"/delta_mw",schemaPath:"#/properties/delta_mw/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.exact_phi_delta !== undefined){
let data3 = data.exact_phi_delta;
const _errs6 = errors;
if((!((typeof data3 == "number") && (isFinite(data3)))) && (data3 !== null)){
validate207.errors = [{instancePath:instancePath+"/exact_phi_delta",schemaPath:"#/properties/exact_phi_delta/type",keyword:"type",params:{type: schema82.properties.exact_phi_delta.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.first_order_error !== undefined){
let data4 = data.first_order_error;
const _errs8 = errors;
if((!((typeof data4 == "number") && (isFinite(data4)))) && (data4 !== null)){
validate207.errors = [{instancePath:instancePath+"/first_order_error",schemaPath:"#/properties/first_order_error/type",keyword:"type",params:{type: schema82.properties.first_order_error.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.first_order_error_rel !== undefined){
let data5 = data.first_order_error_rel;
const _errs10 = errors;
if((!((typeof data5 == "number") && (isFinite(data5)))) && (data5 !== null)){
validate207.errors = [{instancePath:instancePath+"/first_order_error_rel",schemaPath:"#/properties/first_order_error_rel/type",keyword:"type",params:{type: schema82.properties.first_order_error_rel.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.gradient !== undefined){
let data6 = data.gradient;
const _errs12 = errors;
if(errors === _errs12){
if(Array.isArray(data6)){
var valid2 = true;
const len1 = data6.length;
for(let i1=0; i1<len1; i1++){
const _errs14 = errors;
if(!(validate210(data6[i1], {instancePath:instancePath+"/gradient/" + i1,parentData:data6,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate210.errors : vErrors.concat(validate210.errors);
errors = vErrors.length;
}
var valid2 = _errs14 === errors;
if(!valid2){
break;
}
}
}
else {
validate207.errors = [{instancePath:instancePath+"/gradient",schemaPath:"#/properties/gradient/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.predicted_phi_delta !== undefined){
let data8 = data.predicted_phi_delta;
const _errs15 = errors;
if(errors === _errs15){
if(!((typeof data8 == "number") && (isFinite(data8)))){
validate207.errors = [{instancePath:instancePath+"/predicted_phi_delta",schemaPath:"#/properties/predicted_phi_delta/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.reason !== undefined){
const _errs17 = errors;
if(typeof data.reason !== "string"){
validate207.errors = [{instancePath:instancePath+"/reason",schemaPath:"#/properties/reason/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs17 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
else {
validate207.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate207.errors = vErrors;
return errors === 0;
}
validate207.evaluated = {"props":{"accepted":true,"delta_mw":true,"exact_phi_delta":true,"first_order_error":true,"first_order_error_rel":true,"gradient":true,"predicted_phi_delta":true,"reason":true},"dynamicProps":false,"dynamicItems":false};


function validate361(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate361.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((((data.baseline === undefined) && (missing0 = "baseline")) || ((data.exact_verified_result === undefined) && (missing0 = "exact_verified_result"))) || ((data.baseline_phi === undefined) && (missing0 = "baseline_phi"))) || ((data.final_phi === undefined) && (missing0 = "final_phi"))) || ((data.proposal === undefined) && (missing0 = "proposal"))) || ((data.spent_budget_mw === undefined) && (missing0 = "spent_budget_mw"))) || ((data.iterations === undefined) && (missing0 = "iterations"))) || ((data.exact_solves === undefined) && (missing0 = "exact_solves"))){
validate361.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.baseline !== undefined){
const _errs1 = errors;
if(!(validate204(data.baseline, {instancePath:instancePath+"/baseline",parentData:data,parentDataProperty:"baseline",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate204.errors : vErrors.concat(validate204.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.baseline_phi !== undefined){
let data1 = data.baseline_phi;
const _errs2 = errors;
if(errors === _errs2){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate361.errors = [{instancePath:instancePath+"/baseline_phi",schemaPath:"#/properties/baseline_phi/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.exact_solves !== undefined){
let data2 = data.exact_solves;
const _errs4 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate361.errors = [{instancePath:instancePath+"/exact_solves",schemaPath:"#/properties/exact_solves/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs4){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate361.errors = [{instancePath:instancePath+"/exact_solves",schemaPath:"#/properties/exact_solves/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.exact_verified_result !== undefined){
const _errs6 = errors;
if(!(validate204(data.exact_verified_result, {instancePath:instancePath+"/exact_verified_result",parentData:data,parentDataProperty:"exact_verified_result",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate204.errors : vErrors.concat(validate204.errors);
errors = vErrors.length;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.final_phi !== undefined){
let data4 = data.final_phi;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data4 == "number") && (isFinite(data4)))){
validate361.errors = [{instancePath:instancePath+"/final_phi",schemaPath:"#/properties/final_phi/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iterations !== undefined){
let data5 = data.iterations;
const _errs9 = errors;
if(errors === _errs9){
if(Array.isArray(data5)){
var valid1 = true;
const len0 = data5.length;
for(let i0=0; i0<len0; i0++){
const _errs11 = errors;
if(!(validate207(data5[i0], {instancePath:instancePath+"/iterations/" + i0,parentData:data5,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate207.errors : vErrors.concat(validate207.errors);
errors = vErrors.length;
}
var valid1 = _errs11 === errors;
if(!valid1){
break;
}
}
}
else {
validate361.errors = [{instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.proposal !== undefined){
let data7 = data.proposal;
const _errs12 = errors;
if(errors === _errs12){
if(Array.isArray(data7)){
var valid2 = true;
const len1 = data7.length;
for(let i1=0; i1<len1; i1++){
const _errs14 = errors;
if(!(validate208(data7[i1], {instancePath:instancePath+"/proposal/" + i1,parentData:data7,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate208.errors : vErrors.concat(validate208.errors);
errors = vErrors.length;
}
var valid2 = _errs14 === errors;
if(!valid2){
break;
}
}
}
else {
validate361.errors = [{instancePath:instancePath+"/proposal",schemaPath:"#/properties/proposal/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.spent_budget_mw !== undefined){
let data9 = data.spent_budget_mw;
const _errs15 = errors;
if(errors === _errs15){
if(!((typeof data9 == "number") && (isFinite(data9)))){
validate361.errors = [{instancePath:instancePath+"/spent_budget_mw",schemaPath:"#/properties/spent_budget_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
else {
validate361.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate361.errors = vErrors;
return errors === 0;
}
validate361.evaluated = {"props":{"baseline":true,"baseline_phi":true,"exact_solves":true,"exact_verified_result":true,"final_phi":true,"iterations":true,"proposal":true,"spent_budget_mw":true},"dynamicProps":false,"dynamicItems":false};

export const plan_spec = validate366;
const schema85 = {"description":"The bounded decision space and search limits of one planning run. All\npower quantities are MW.","properties":{"budget_mw":{"description":"Total intervention budget: `Σ_e increase_e` never exceeds it.","exclusiveMinimum":0,"format":"double","type":"number"},"candidates":{"description":"Candidate branches whose thermal ratings the search may increase.","items":{"type":"string"},"minItems":1,"type":"array"},"exact_solve_budget":{"description":"Maximum number of exact OPF solves, including the baseline and every\naccepted or rejected trial.","format":"uint","minimum":1,"type":"integer"},"increment_mw":{"description":"Discrete capacity increase considered in one trial.","exclusiveMinimum":0,"format":"double","type":"number"},"max_changed_lines":{"description":"Positive cardinality bound over the final proposal, not one iteration.","format":"uint","minimum":1,"type":"integer"},"max_increase_per_branch_mw":{"description":"Maximum cumulative increase for one branch.","exclusiveMinimum":0,"format":"double","type":"number"},"objective":{"$ref":"#/$defs/ImplicitObjective"}},"required":["objective","candidates","max_increase_per_branch_mw","budget_mw","increment_mw","max_changed_lines","exact_solve_budget"],"type":"object"};
const schema86 = {"description":"A serializable outer objective over the optimal solve. Every\nvariant is a scalar with a well defined gradient through the KKT system;\nnew observables (generation, line loading, operating cost) add variants\nrather than accepting code.","oneOf":[{"description":"`Phi = Σ_b w_b · lmp_b` over the selected buses. Positive weights\nmake the search look for capacity increases that lower the weighted\nmarginal value of demand.","properties":{"kind":{"const":"weighted_lmp","type":"string"},"weights":{"items":{"$ref":"#/$defs/BusWeight"},"minItems":1,"type":"array"}},"required":["kind","weights"],"type":"object"}]};
const schema87 = {"description":"One bus term of a weighted objective, addressed by PowerIO bus ID.","properties":{"bus":{"$ref":"#/$defs/BusId"},"weight":{"format":"double","type":"number"}},"required":["bus","weight"],"type":"object"};
const schema88 = {"description":"A source bus ID, preserved from the input format.\n\nMATPOWER IDs are 1-based and can contain gaps. They are distinct from the\nzero based dense indices produced by\n[`IndexedNetwork::bus_index`](crate::IndexedNetwork::bus_index). JSON stores\nthis type as an integer.","format":"uint","minimum":0,"type":"integer"};

function validate218(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate218.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(!(((typeof data == "number") && (!(data % 1) && !isNaN(data))) && (isFinite(data)))){
validate218.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === 0){
if((typeof data == "number") && (isFinite(data))){
if(data < 0 || isNaN(data)){
validate218.errors = [{instancePath,schemaPath:"#/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
validate218.errors = vErrors;
return errors === 0;
}
validate218.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate217(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate217.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.bus === undefined) && (missing0 = "bus")) || ((data.weight === undefined) && (missing0 = "weight"))){
validate217.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
const _errs1 = errors;
if(!(validate218(data.bus, {instancePath:instancePath+"/bus",parentData:data,parentDataProperty:"bus",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate218.errors : vErrors.concat(validate218.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.weight !== undefined){
let data1 = data.weight;
const _errs2 = errors;
if(errors === _errs2){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate217.errors = [{instancePath:instancePath+"/weight",schemaPath:"#/properties/weight/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate217.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate217.errors = vErrors;
return errors === 0;
}
validate217.evaluated = {"props":{"bus":true,"weight":true},"dynamicProps":false,"dynamicItems":false};


function validate216(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate216.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(errors === _errs1){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.kind === undefined) && (missing0 = "kind")) || ((data.weights === undefined) && (missing0 = "weights"))){
const err0 = {instancePath,schemaPath:"#/oneOf/0/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
else {
if(data.kind !== undefined){
let data0 = data.kind;
const _errs3 = errors;
if(typeof data0 !== "string"){
const err1 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/0/properties/kind/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
if("weighted_lmp" !== data0){
const err2 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/0/properties/kind/const",keyword:"const",params:{allowedValue: "weighted_lmp"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var valid1 = _errs3 === errors;
}
else {
var valid1 = true;
}
if(valid1){
if(data.weights !== undefined){
let data1 = data.weights;
const _errs5 = errors;
if(errors === _errs5){
if(Array.isArray(data1)){
if(data1.length < 1){
const err3 = {instancePath:instancePath+"/weights",schemaPath:"#/oneOf/0/properties/weights/minItems",keyword:"minItems",params:{limit: 1},message:"must NOT have fewer than 1 items"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
else {
var valid2 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
const _errs7 = errors;
if(!(validate217(data1[i0], {instancePath:instancePath+"/weights/" + i0,parentData:data1,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate217.errors : vErrors.concat(validate217.errors);
errors = vErrors.length;
}
var valid2 = _errs7 === errors;
if(!valid2){
break;
}
}
}
}
else {
const err4 = {instancePath:instancePath+"/weights",schemaPath:"#/oneOf/0/properties/weights/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
var valid1 = _errs5 === errors;
}
else {
var valid1 = true;
}
}
}
}
else {
const err5 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
var props0 = {};
props0.kind = true;
props0.weights = true;
}
if(!valid0){
const err6 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
validate216.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate216.errors = vErrors;
evaluated0.props = props0;
return errors === 0;
}
validate216.evaluated = {"dynamicProps":true,"dynamicItems":false};


function validate366(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate366.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.objective === undefined) && (missing0 = "objective")) || ((data.candidates === undefined) && (missing0 = "candidates"))) || ((data.max_increase_per_branch_mw === undefined) && (missing0 = "max_increase_per_branch_mw"))) || ((data.budget_mw === undefined) && (missing0 = "budget_mw"))) || ((data.increment_mw === undefined) && (missing0 = "increment_mw"))) || ((data.max_changed_lines === undefined) && (missing0 = "max_changed_lines"))) || ((data.exact_solve_budget === undefined) && (missing0 = "exact_solve_budget"))){
validate366.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.budget_mw !== undefined){
let data0 = data.budget_mw;
const _errs1 = errors;
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 <= 0 || isNaN(data0)){
validate366.errors = [{instancePath:instancePath+"/budget_mw",schemaPath:"#/properties/budget_mw/exclusiveMinimum",keyword:"exclusiveMinimum",params:{comparison: ">", limit: 0},message:"must be > 0"}];
return false;
}
}
else {
validate366.errors = [{instancePath:instancePath+"/budget_mw",schemaPath:"#/properties/budget_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.candidates !== undefined){
let data1 = data.candidates;
const _errs3 = errors;
if(errors === _errs3){
if(Array.isArray(data1)){
if(data1.length < 1){
validate366.errors = [{instancePath:instancePath+"/candidates",schemaPath:"#/properties/candidates/minItems",keyword:"minItems",params:{limit: 1},message:"must NOT have fewer than 1 items"}];
return false;
}
else {
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
const _errs5 = errors;
if(typeof data1[i0] !== "string"){
validate366.errors = [{instancePath:instancePath+"/candidates/" + i0,schemaPath:"#/properties/candidates/items/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid1 = _errs5 === errors;
if(!valid1){
break;
}
}
}
}
else {
validate366.errors = [{instancePath:instancePath+"/candidates",schemaPath:"#/properties/candidates/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.exact_solve_budget !== undefined){
let data3 = data.exact_solve_budget;
const _errs7 = errors;
if(!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))){
validate366.errors = [{instancePath:instancePath+"/exact_solve_budget",schemaPath:"#/properties/exact_solve_budget/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs7){
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 1 || isNaN(data3)){
validate366.errors = [{instancePath:instancePath+"/exact_solve_budget",schemaPath:"#/properties/exact_solve_budget/minimum",keyword:"minimum",params:{comparison: ">=", limit: 1},message:"must be >= 1"}];
return false;
}
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.increment_mw !== undefined){
let data4 = data.increment_mw;
const _errs9 = errors;
if(errors === _errs9){
if((typeof data4 == "number") && (isFinite(data4))){
if(data4 <= 0 || isNaN(data4)){
validate366.errors = [{instancePath:instancePath+"/increment_mw",schemaPath:"#/properties/increment_mw/exclusiveMinimum",keyword:"exclusiveMinimum",params:{comparison: ">", limit: 0},message:"must be > 0"}];
return false;
}
}
else {
validate366.errors = [{instancePath:instancePath+"/increment_mw",schemaPath:"#/properties/increment_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.max_changed_lines !== undefined){
let data5 = data.max_changed_lines;
const _errs11 = errors;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
validate366.errors = [{instancePath:instancePath+"/max_changed_lines",schemaPath:"#/properties/max_changed_lines/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs11){
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 1 || isNaN(data5)){
validate366.errors = [{instancePath:instancePath+"/max_changed_lines",schemaPath:"#/properties/max_changed_lines/minimum",keyword:"minimum",params:{comparison: ">=", limit: 1},message:"must be >= 1"}];
return false;
}
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.max_increase_per_branch_mw !== undefined){
let data6 = data.max_increase_per_branch_mw;
const _errs13 = errors;
if(errors === _errs13){
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 <= 0 || isNaN(data6)){
validate366.errors = [{instancePath:instancePath+"/max_increase_per_branch_mw",schemaPath:"#/properties/max_increase_per_branch_mw/exclusiveMinimum",keyword:"exclusiveMinimum",params:{comparison: ">", limit: 0},message:"must be > 0"}];
return false;
}
}
else {
validate366.errors = [{instancePath:instancePath+"/max_increase_per_branch_mw",schemaPath:"#/properties/max_increase_per_branch_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
const _errs15 = errors;
if(!(validate216(data.objective, {instancePath:instancePath+"/objective",parentData:data,parentDataProperty:"objective",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate216.errors : vErrors.concat(validate216.errors);
errors = vErrors.length;
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate366.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate366.errors = vErrors;
return errors === 0;
}
validate366.evaluated = {"props":{"budget_mw":true,"candidates":true,"exact_solve_budget":true,"increment_mw":true,"max_changed_lines":true,"max_increase_per_branch_mw":true,"objective":true},"dynamicProps":false,"dynamicItems":false};

export const preview = validate368;
const schema89 = {"description":"A first-order preview of an edit at the committed operating point: the predicted\nchange in each watched operand, the predicted objective change, and the\nlinearization caveat.","properties":{"local_only":{"description":"Always `true`: a continuous edit's preview is a local linearization, valid only\nuntil a binding constraint changes. [`commit`](Study::commit) is the truth.","type":"boolean"},"objective_delta":{"description":"First-order change in the declared objective along the edit. `None` for power\nflow or a feasibility objective. For a demand edit this is the committed\nmarginal objective value dotted with the demand step.","format":"double","type":["number","null"]},"operands":{"description":"One predicted operand-delta column per watched operand, in request order.","items":{"$ref":"#/$defs/PreviewColumn"},"type":"array"}},"required":["operands","local_only"],"type":"object"};
const schema90 = {"description":"The predicted change in one operand across the elements it ranges over.","properties":{"operand":{"$ref":"#/$defs/Operand"},"units":{"description":"Served unit label of the prediction (for example `objective_unit/MW` or `pu`).","type":"string"},"values":{"items":{"$ref":"#/$defs/PreviewValue"},"type":"array"}},"required":["operand","values","units"],"type":"object"};
const schema91 = {"description":"One element's predicted operand change, keyed by its source element id.","properties":{"element":{"$ref":"#/$defs/ElementId"},"index":{"format":"uint","minimum":0,"type":"integer"},"value":{"format":"double","type":"number"}},"required":["element","index","value"],"type":"object"};

function validate226(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate226.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.element === undefined) && (missing0 = "element")) || ((data.index === undefined) && (missing0 = "index"))) || ((data.value === undefined) && (missing0 = "value"))){
validate226.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.element !== undefined){
const _errs1 = errors;
if(!(validate68(data.element, {instancePath:instancePath+"/element",parentData:data,parentDataProperty:"element",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate68.errors : vErrors.concat(validate68.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.index !== undefined){
let data1 = data.index;
const _errs2 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate226.errors = [{instancePath:instancePath+"/index",schemaPath:"#/properties/index/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs2){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate226.errors = [{instancePath:instancePath+"/index",schemaPath:"#/properties/index/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data2 = data.value;
const _errs4 = errors;
if(errors === _errs4){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate226.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate226.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate226.errors = vErrors;
return errors === 0;
}
validate226.evaluated = {"props":{"element":true,"index":true,"value":true},"dynamicProps":false,"dynamicItems":false};


function validate224(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate224.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.operand === undefined) && (missing0 = "operand")) || ((data.values === undefined) && (missing0 = "values"))) || ((data.units === undefined) && (missing0 = "units"))){
validate224.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.operand !== undefined){
const _errs1 = errors;
if(!(validate88(data.operand, {instancePath:instancePath+"/operand",parentData:data,parentDataProperty:"operand",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate88.errors : vErrors.concat(validate88.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.units !== undefined){
const _errs2 = errors;
if(typeof data.units !== "string"){
validate224.errors = [{instancePath:instancePath+"/units",schemaPath:"#/properties/units/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.values !== undefined){
let data2 = data.values;
const _errs4 = errors;
if(errors === _errs4){
if(Array.isArray(data2)){
var valid1 = true;
const len0 = data2.length;
for(let i0=0; i0<len0; i0++){
const _errs6 = errors;
if(!(validate226(data2[i0], {instancePath:instancePath+"/values/" + i0,parentData:data2,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate226.errors : vErrors.concat(validate226.errors);
errors = vErrors.length;
}
var valid1 = _errs6 === errors;
if(!valid1){
break;
}
}
}
else {
validate224.errors = [{instancePath:instancePath+"/values",schemaPath:"#/properties/values/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate224.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate224.errors = vErrors;
return errors === 0;
}
validate224.evaluated = {"props":{"operand":true,"units":true,"values":true},"dynamicProps":false,"dynamicItems":false};


function validate368(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate368.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.operands === undefined) && (missing0 = "operands")) || ((data.local_only === undefined) && (missing0 = "local_only"))){
validate368.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.local_only !== undefined){
const _errs1 = errors;
if(typeof data.local_only !== "boolean"){
validate368.errors = [{instancePath:instancePath+"/local_only",schemaPath:"#/properties/local_only/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective_delta !== undefined){
let data1 = data.objective_delta;
const _errs3 = errors;
if((!((typeof data1 == "number") && (isFinite(data1)))) && (data1 !== null)){
validate368.errors = [{instancePath:instancePath+"/objective_delta",schemaPath:"#/properties/objective_delta/type",keyword:"type",params:{type: schema89.properties.objective_delta.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.operands !== undefined){
let data2 = data.operands;
const _errs5 = errors;
if(errors === _errs5){
if(Array.isArray(data2)){
var valid1 = true;
const len0 = data2.length;
for(let i0=0; i0<len0; i0++){
const _errs7 = errors;
if(!(validate224(data2[i0], {instancePath:instancePath+"/operands/" + i0,parentData:data2,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate224.errors : vErrors.concat(validate224.errors);
errors = vErrors.length;
}
var valid1 = _errs7 === errors;
if(!valid1){
break;
}
}
}
else {
validate368.errors = [{instancePath:instancePath+"/operands",schemaPath:"#/properties/operands/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate368.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate368.errors = vErrors;
return errors === 0;
}
validate368.evaluated = {"props":{"local_only":true,"objective_delta":true,"operands":true},"dynamicProps":false,"dynamicItems":false};

export const sensitivity = validate370;
const schema92 = {"description":"One requested sensitivity cell: an [`Operand`] differentiated with respect to a\n[`Parameter`], over an optional parameter-index subset, in an optional direction.\nThe operand/parameter are the public serde-tagged enums verbatim\n(`{\"Price\":\"Active\"}` / `{\"Demand\":\"Active\"}`).","properties":{"indices":{"default":null,"description":"Dense parameter-column indices; `None` computes the whole axis.","items":{"format":"uint","minimum":0,"type":"integer"},"type":["array","null"]},"mode":{"$ref":"#/$defs/Mode","default":"Auto","description":"Forward / Adjoint / Auto. `Auto` when omitted."},"operand":{"$ref":"#/$defs/Operand"},"parameter":{"$ref":"#/$defs/Parameter"}},"required":["operand","parameter"],"type":"object"};
const schema93 = {"description":"Which linear systems the engine factorizes. [`Forward`](Mode::Forward) and\n[`Adjoint`](Mode::Adjoint) return the same matrix; the choice trades a\nfactorization of `K` (cheap when parameters are few) for one of `K'` (cheap when\noperands are few). [`Auto`](Mode::Auto) picks the smaller dimension and is the api\ndefault.","oneOf":[{"const":"Forward","description":"Solve `K X = rhs` once per parameter set, read the operand rows.","type":"string"},{"const":"Adjoint","description":"Solve `K' Y = S'` once per operand set, contract with `rhs`.","type":"string"},{"const":"Auto","description":"Pick [`Forward`](Mode::Forward) when parameters are at most operands, else\n[`Adjoint`](Mode::Adjoint).","type":"string"}]};

function validate232(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate232.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("Forward" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "Forward"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("Adjoint" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "Adjoint"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
const _errs5 = errors;
if(typeof data !== "string"){
const err4 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if("Auto" !== data){
const err5 = {instancePath,schemaPath:"#/oneOf/2/const",keyword:"const",params:{allowedValue: "Auto"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
}
}
}
if(!valid0){
const err6 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
validate232.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate232.errors = vErrors;
return errors === 0;
}
validate232.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate370(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate370.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.operand === undefined) && (missing0 = "operand")) || ((data.parameter === undefined) && (missing0 = "parameter"))){
validate370.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.indices !== undefined){
let data0 = data.indices;
const _errs1 = errors;
if((!(Array.isArray(data0))) && (data0 !== null)){
validate370.errors = [{instancePath:instancePath+"/indices",schemaPath:"#/properties/indices/type",keyword:"type",params:{type: schema92.properties.indices.type},message:"must be array,null"}];
return false;
}
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
let data1 = data0[i0];
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate370.errors = [{instancePath:instancePath+"/indices/" + i0,schemaPath:"#/properties/indices/items/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate370.errors = [{instancePath:instancePath+"/indices/" + i0,schemaPath:"#/properties/indices/items/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.mode !== undefined){
const _errs5 = errors;
if(!(validate232(data.mode, {instancePath:instancePath+"/mode",parentData:data,parentDataProperty:"mode",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate232.errors : vErrors.concat(validate232.errors);
errors = vErrors.length;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.operand !== undefined){
const _errs6 = errors;
if(!(validate88(data.operand, {instancePath:instancePath+"/operand",parentData:data,parentDataProperty:"operand",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate88.errors : vErrors.concat(validate88.errors);
errors = vErrors.length;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.parameter !== undefined){
const _errs7 = errors;
if(!(validate70(data.parameter, {instancePath:instancePath+"/parameter",parentData:data,parentDataProperty:"parameter",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate70.errors : vErrors.concat(validate70.errors);
errors = vErrors.length;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate370.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate370.errors = vErrors;
return errors === 0;
}
validate370.evaluated = {"props":{"indices":true,"mode":true,"operand":true,"parameter":true},"dynamicProps":false,"dynamicItems":false};

export const settings = validate374;

function validate374(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate374.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((data.version === undefined) && (missing0 = "version")) || ((data.calculation === undefined) && (missing0 = "calculation"))) || ((data.engine_defaults === undefined) && (missing0 = "engine_defaults"))) || ((data.baseline === undefined) && (missing0 = "baseline"))) || ((data.overrides === undefined) && (missing0 = "overrides"))) || ((data.effective === undefined) && (missing0 = "effective"))){
validate374.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.baseline !== undefined){
let data0 = data.baseline;
const _errs1 = errors;
if(errors === _errs1){
if(data0 && typeof data0 == "object" && !Array.isArray(data0)){
}
else {
validate374.errors = [{instancePath:instancePath+"/baseline",schemaPath:"#/properties/baseline/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.calculation !== undefined){
const _errs4 = errors;
if(!(validate55(data.calculation, {instancePath:instancePath+"/calculation",parentData:data,parentDataProperty:"calculation",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate55.errors : vErrors.concat(validate55.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.effective !== undefined){
let data2 = data.effective;
const _errs5 = errors;
if(errors === _errs5){
if(data2 && typeof data2 == "object" && !Array.isArray(data2)){
}
else {
validate374.errors = [{instancePath:instancePath+"/effective",schemaPath:"#/properties/effective/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.engine_defaults !== undefined){
let data3 = data.engine_defaults;
const _errs8 = errors;
if(errors === _errs8){
if(data3 && typeof data3 == "object" && !Array.isArray(data3)){
}
else {
validate374.errors = [{instancePath:instancePath+"/engine_defaults",schemaPath:"#/properties/engine_defaults/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.overrides !== undefined){
let data4 = data.overrides;
const _errs11 = errors;
if(errors === _errs11){
if(data4 && typeof data4 == "object" && !Array.isArray(data4)){
}
else {
validate374.errors = [{instancePath:instancePath+"/overrides",schemaPath:"#/properties/overrides/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.version !== undefined){
let data5 = data.version;
const _errs14 = errors;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
validate374.errors = [{instancePath:instancePath+"/version",schemaPath:"#/properties/version/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs14){
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
validate374.errors = [{instancePath:instancePath+"/version",schemaPath:"#/properties/version/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs14 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
else {
validate374.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate374.errors = vErrors;
return errors === 0;
}
validate374.evaluated = {"props":{"baseline":true,"calculation":true,"effective":true,"engine_defaults":true,"overrides":true,"version":true},"dynamicProps":false,"dynamicItems":false};

export const solve = validate376;

function validate376(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate376.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.formulation === undefined) && (missing0 = "formulation")) || ((data.status === undefined) && (missing0 = "status"))){
validate376.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch_constraints !== undefined){
let data0 = data.branch_constraints;
const _errs1 = errors;
if((!(Array.isArray(data0))) && (data0 !== null)){
validate376.errors = [{instancePath:instancePath+"/branch_constraints",schemaPath:"#/properties/branch_constraints/type",keyword:"type",params:{type: schema40.properties.branch_constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate100(data0[i0], {instancePath:instancePath+"/branch_constraints/" + i0,parentData:data0,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate100.errors : vErrors.concat(validate100.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.branch_susceptance_formula !== undefined){
let data2 = data.branch_susceptance_formula;
const _errs4 = errors;
const _errs5 = errors;
let valid2 = false;
const _errs6 = errors;
if(!(validate102(data2, {instancePath:instancePath+"/branch_susceptance_formula",parentData:data,parentDataProperty:"branch_susceptance_formula",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate102.errors : vErrors.concat(validate102.errors);
errors = vErrors.length;
}
var _valid0 = _errs6 === errors;
valid2 = valid2 || _valid0;
const _errs7 = errors;
if(data2 !== null){
const err0 = {instancePath:instancePath+"/branch_susceptance_formula",schemaPath:"#/properties/branch_susceptance_formula/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs7 === errors;
valid2 = valid2 || _valid0;
if(!valid2){
const err1 = {instancePath:instancePath+"/branch_susceptance_formula",schemaPath:"#/properties/branch_susceptance_formula/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate376.errors = vErrors;
return false;
}
else {
errors = _errs5;
if(vErrors !== null){
if(_errs5){
vErrors.length = _errs5;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.constraints !== undefined){
let data3 = data.constraints;
const _errs9 = errors;
if((!(Array.isArray(data3))) && (data3 !== null)){
validate376.errors = [{instancePath:instancePath+"/constraints",schemaPath:"#/properties/constraints/type",keyword:"type",params:{type: schema40.properties.constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs9){
if(Array.isArray(data3)){
var valid3 = true;
const len1 = data3.length;
for(let i1=0; i1<len1; i1++){
const _errs11 = errors;
if(!(validate104(data3[i1], {instancePath:instancePath+"/constraints/" + i1,parentData:data3,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate104.errors : vErrors.concat(validate104.errors);
errors = vErrors.length;
}
var valid3 = _errs11 === errors;
if(!valid3){
break;
}
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.diagnostics !== undefined){
let data5 = data.diagnostics;
const _errs12 = errors;
if(errors === _errs12){
if(Array.isArray(data5)){
var valid4 = true;
const len2 = data5.length;
for(let i2=0; i2<len2; i2++){
const _errs14 = errors;
if(!(validate106(data5[i2], {instancePath:instancePath+"/diagnostics/" + i2,parentData:data5,parentDataProperty:i2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate106.errors : vErrors.concat(validate106.errors);
errors = vErrors.length;
}
var valid4 = _errs14 === errors;
if(!valid4){
break;
}
}
}
else {
validate376.errors = [{instancePath:instancePath+"/diagnostics",schemaPath:"#/properties/diagnostics/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.dispatch !== undefined){
let data7 = data.dispatch;
const _errs15 = errors;
if((!(Array.isArray(data7))) && (data7 !== null)){
validate376.errors = [{instancePath:instancePath+"/dispatch",schemaPath:"#/properties/dispatch/type",keyword:"type",params:{type: schema40.properties.dispatch.type},message:"must be array,null"}];
return false;
}
if(errors === _errs15){
if(Array.isArray(data7)){
var valid5 = true;
const len3 = data7.length;
for(let i3=0; i3<len3; i3++){
const _errs17 = errors;
if(!(validate108(data7[i3], {instancePath:instancePath+"/dispatch/" + i3,parentData:data7,parentDataProperty:i3,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate108.errors : vErrors.concat(validate108.errors);
errors = vErrors.length;
}
var valid5 = _errs17 === errors;
if(!valid5){
break;
}
}
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.flows !== undefined){
let data9 = data.flows;
const _errs18 = errors;
if((!(Array.isArray(data9))) && (data9 !== null)){
validate376.errors = [{instancePath:instancePath+"/flows",schemaPath:"#/properties/flows/type",keyword:"type",params:{type: schema40.properties.flows.type},message:"must be array,null"}];
return false;
}
if(errors === _errs18){
if(Array.isArray(data9)){
var valid6 = true;
const len4 = data9.length;
for(let i4=0; i4<len4; i4++){
const _errs20 = errors;
if(!(validate110(data9[i4], {instancePath:instancePath+"/flows/" + i4,parentData:data9,parentDataProperty:i4,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate110.errors : vErrors.concat(validate110.errors);
errors = vErrors.length;
}
var valid6 = _errs20 === errors;
if(!valid6){
break;
}
}
}
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.formulation !== undefined){
const _errs21 = errors;
if(!(validate120(data.formulation, {instancePath:instancePath+"/formulation",parentData:data,parentDataProperty:"formulation",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate120.errors : vErrors.concat(validate120.errors);
errors = vErrors.length;
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.initialization !== undefined){
let data12 = data.initialization;
const _errs22 = errors;
const _errs23 = errors;
let valid7 = false;
const _errs24 = errors;
if(!(validate122(data12, {instancePath:instancePath+"/initialization",parentData:data,parentDataProperty:"initialization",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate122.errors : vErrors.concat(validate122.errors);
errors = vErrors.length;
}
var _valid1 = _errs24 === errors;
valid7 = valid7 || _valid1;
if(_valid1){
var props0 = {};
props0.attempts = true;
props0.requested = true;
props0.restart = true;
props0.used = true;
}
const _errs25 = errors;
if(data12 !== null){
const err2 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs25 === errors;
valid7 = valid7 || _valid1;
if(!valid7){
const err3 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate376.errors = vErrors;
return false;
}
else {
errors = _errs23;
if(vErrors !== null){
if(_errs23){
vErrors.length = _errs23;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs22 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.injections !== undefined){
let data13 = data.injections;
const _errs27 = errors;
if((!(Array.isArray(data13))) && (data13 !== null)){
validate376.errors = [{instancePath:instancePath+"/injections",schemaPath:"#/properties/injections/type",keyword:"type",params:{type: schema40.properties.injections.type},message:"must be array,null"}];
return false;
}
if(errors === _errs27){
if(Array.isArray(data13)){
var valid8 = true;
const len5 = data13.length;
for(let i5=0; i5<len5; i5++){
const _errs29 = errors;
if(!(validate128(data13[i5], {instancePath:instancePath+"/injections/" + i5,parentData:data13,parentDataProperty:i5,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate128.errors : vErrors.concat(validate128.errors);
errors = vErrors.length;
}
var valid8 = _errs29 === errors;
if(!valid8){
break;
}
}
}
}
var valid0 = _errs27 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iterations !== undefined){
let data15 = data.iterations;
const _errs30 = errors;
const _errs31 = errors;
let valid9 = false;
const _errs32 = errors;
if(!(validate60(data15, {instancePath:instancePath+"/iterations",parentData:data,parentDataProperty:"iterations",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate60.errors : vErrors.concat(validate60.errors);
errors = vErrors.length;
}
var _valid2 = _errs32 === errors;
valid9 = valid9 || _valid2;
const _errs33 = errors;
if(data15 !== null){
const err4 = {instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
var _valid2 = _errs33 === errors;
valid9 = valid9 || _valid2;
if(!valid9){
const err5 = {instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
validate376.errors = vErrors;
return false;
}
else {
errors = _errs31;
if(vErrors !== null){
if(_errs31){
vErrors.length = _errs31;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs30 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.limit_rounds !== undefined){
let data16 = data.limit_rounds;
const _errs35 = errors;
if((!(Array.isArray(data16))) && (data16 !== null)){
validate376.errors = [{instancePath:instancePath+"/limit_rounds",schemaPath:"#/properties/limit_rounds/type",keyword:"type",params:{type: schema40.properties.limit_rounds.type},message:"must be array,null"}];
return false;
}
if(errors === _errs35){
if(Array.isArray(data16)){
var valid10 = true;
const len6 = data16.length;
for(let i6=0; i6<len6; i6++){
const _errs37 = errors;
if(!(validate131(data16[i6], {instancePath:instancePath+"/limit_rounds/" + i6,parentData:data16,parentDataProperty:i6,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate131.errors : vErrors.concat(validate131.errors);
errors = vErrors.length;
}
var valid10 = _errs37 === errors;
if(!valid10){
break;
}
}
}
}
var valid0 = _errs35 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lmp !== undefined){
let data18 = data.lmp;
const _errs38 = errors;
if((!(Array.isArray(data18))) && (data18 !== null)){
validate376.errors = [{instancePath:instancePath+"/lmp",schemaPath:"#/properties/lmp/type",keyword:"type",params:{type: schema40.properties.lmp.type},message:"must be array,null"}];
return false;
}
if(errors === _errs38){
if(Array.isArray(data18)){
var valid11 = true;
const len7 = data18.length;
for(let i7=0; i7<len7; i7++){
const _errs40 = errors;
if(!(validate133(data18[i7], {instancePath:instancePath+"/lmp/" + i7,parentData:data18,parentDataProperty:i7,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid11 = _errs40 === errors;
if(!valid11){
break;
}
}
}
}
var valid0 = _errs38 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lmp_q !== undefined){
let data20 = data.lmp_q;
const _errs41 = errors;
if((!(Array.isArray(data20))) && (data20 !== null)){
validate376.errors = [{instancePath:instancePath+"/lmp_q",schemaPath:"#/properties/lmp_q/type",keyword:"type",params:{type: schema40.properties.lmp_q.type},message:"must be array,null"}];
return false;
}
if(errors === _errs41){
if(Array.isArray(data20)){
var valid12 = true;
const len8 = data20.length;
for(let i8=0; i8<len8; i8++){
const _errs43 = errors;
if(!(validate133(data20[i8], {instancePath:instancePath+"/lmp_q/" + i8,parentData:data20,parentDataProperty:i8,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid12 = _errs43 === errors;
if(!valid12){
break;
}
}
}
}
var valid0 = _errs41 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
let data22 = data.objective;
const _errs44 = errors;
if((!((typeof data22 == "number") && (isFinite(data22)))) && (data22 !== null)){
validate376.errors = [{instancePath:instancePath+"/objective",schemaPath:"#/properties/objective/type",keyword:"type",params:{type: schema40.properties.objective.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs44 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective_kind !== undefined){
let data23 = data.objective_kind;
const _errs46 = errors;
const _errs47 = errors;
let valid13 = false;
const _errs48 = errors;
if(!(validate136(data23, {instancePath:instancePath+"/objective_kind",parentData:data,parentDataProperty:"objective_kind",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate136.errors : vErrors.concat(validate136.errors);
errors = vErrors.length;
}
var _valid3 = _errs48 === errors;
valid13 = valid13 || _valid3;
const _errs49 = errors;
if(data23 !== null){
const err6 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
var _valid3 = _errs49 === errors;
valid13 = valid13 || _valid3;
if(!valid13){
const err7 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
validate376.errors = vErrors;
return false;
}
else {
errors = _errs47;
if(vErrors !== null){
if(_errs47){
vErrors.length = _errs47;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs46 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.sensitivities !== undefined){
let data24 = data.sensitivities;
const _errs51 = errors;
if(errors === _errs51){
if(Array.isArray(data24)){
var valid14 = true;
const len9 = data24.length;
for(let i9=0; i9<len9; i9++){
const _errs53 = errors;
if(!(validate66(data24[i9], {instancePath:instancePath+"/sensitivities/" + i9,parentData:data24,parentDataProperty:i9,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate66.errors : vErrors.concat(validate66.errors);
errors = vErrors.length;
}
var valid14 = _errs53 === errors;
if(!valid14){
break;
}
}
}
else {
validate376.errors = [{instancePath:instancePath+"/sensitivities",schemaPath:"#/properties/sensitivities/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs51 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.status !== undefined){
const _errs54 = errors;
if(!(validate139(data.status, {instancePath:instancePath+"/status",parentData:data,parentDataProperty:"status",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate139.errors : vErrors.concat(validate139.errors);
errors = vErrors.length;
}
var valid0 = _errs54 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.va !== undefined){
let data27 = data.va;
const _errs55 = errors;
if((!(Array.isArray(data27))) && (data27 !== null)){
validate376.errors = [{instancePath:instancePath+"/va",schemaPath:"#/properties/va/type",keyword:"type",params:{type: schema40.properties.va.type},message:"must be array,null"}];
return false;
}
if(errors === _errs55){
if(Array.isArray(data27)){
var valid15 = true;
const len10 = data27.length;
for(let i10=0; i10<len10; i10++){
const _errs57 = errors;
if(!(validate133(data27[i10], {instancePath:instancePath+"/va/" + i10,parentData:data27,parentDataProperty:i10,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid15 = _errs57 === errors;
if(!valid15){
break;
}
}
}
}
var valid0 = _errs55 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.vm !== undefined){
let data29 = data.vm;
const _errs58 = errors;
if((!(Array.isArray(data29))) && (data29 !== null)){
validate376.errors = [{instancePath:instancePath+"/vm",schemaPath:"#/properties/vm/type",keyword:"type",params:{type: schema40.properties.vm.type},message:"must be array,null"}];
return false;
}
if(errors === _errs58){
if(Array.isArray(data29)){
var valid16 = true;
const len11 = data29.length;
for(let i11=0; i11<len11; i11++){
const _errs60 = errors;
if(!(validate133(data29[i11], {instancePath:instancePath+"/vm/" + i11,parentData:data29,parentDataProperty:i11,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid16 = _errs60 === errors;
if(!valid16){
break;
}
}
}
}
var valid0 = _errs58 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.w !== undefined){
let data31 = data.w;
const _errs61 = errors;
if((!(Array.isArray(data31))) && (data31 !== null)){
validate376.errors = [{instancePath:instancePath+"/w",schemaPath:"#/properties/w/type",keyword:"type",params:{type: schema40.properties.w.type},message:"must be array,null"}];
return false;
}
if(errors === _errs61){
if(Array.isArray(data31)){
var valid17 = true;
const len12 = data31.length;
for(let i12=0; i12<len12; i12++){
const _errs63 = errors;
if(!(validate133(data31[i12], {instancePath:instancePath+"/w/" + i12,parentData:data31,parentDataProperty:i12,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid17 = _errs63 === errors;
if(!valid17){
break;
}
}
}
}
var valid0 = _errs61 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.wi !== undefined){
let data33 = data.wi;
const _errs64 = errors;
if((!(Array.isArray(data33))) && (data33 !== null)){
validate376.errors = [{instancePath:instancePath+"/wi",schemaPath:"#/properties/wi/type",keyword:"type",params:{type: schema40.properties.wi.type},message:"must be array,null"}];
return false;
}
if(errors === _errs64){
if(Array.isArray(data33)){
var valid18 = true;
const len13 = data33.length;
for(let i13=0; i13<len13; i13++){
const _errs66 = errors;
if(!(validate144(data33[i13], {instancePath:instancePath+"/wi/" + i13,parentData:data33,parentDataProperty:i13,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate144.errors : vErrors.concat(validate144.errors);
errors = vErrors.length;
}
var valid18 = _errs66 === errors;
if(!valid18){
break;
}
}
}
}
var valid0 = _errs64 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.wr !== undefined){
let data35 = data.wr;
const _errs67 = errors;
if((!(Array.isArray(data35))) && (data35 !== null)){
validate376.errors = [{instancePath:instancePath+"/wr",schemaPath:"#/properties/wr/type",keyword:"type",params:{type: schema40.properties.wr.type},message:"must be array,null"}];
return false;
}
if(errors === _errs67){
if(Array.isArray(data35)){
var valid19 = true;
const len14 = data35.length;
for(let i14=0; i14<len14; i14++){
const _errs69 = errors;
if(!(validate144(data35[i14], {instancePath:instancePath+"/wr/" + i14,parentData:data35,parentDataProperty:i14,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate144.errors : vErrors.concat(validate144.errors);
errors = vErrors.length;
}
var valid19 = _errs69 === errors;
if(!valid19){
break;
}
}
}
}
var valid0 = _errs67 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate376.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate376.errors = vErrors;
return errors === 0;
}
validate376.evaluated = {"props":{"branch_constraints":true,"branch_susceptance_formula":true,"constraints":true,"diagnostics":true,"dispatch":true,"flows":true,"formulation":true,"initialization":true,"injections":true,"iterations":true,"limit_rounds":true,"lmp":true,"lmp_q":true,"objective":true,"objective_kind":true,"sensitivities":true,"status":true,"va":true,"vm":true,"w":true,"wi":true,"wr":true},"dynamicProps":false,"dynamicItems":false};

export const study_bundle = validate398;
const schema94 = {"additionalProperties":false,"description":"Approvals are intentionally absent from the portable document.","properties":{"artifacts":{"additionalProperties":{"$ref":"#/$defs/StudyArtifact"},"type":"object"},"document":{"$ref":"#/$defs/StudyDocument"}},"required":["document","artifacts"],"type":"object"};
const schema95 = {"additionalProperties":false,"properties":{"kind":{"$ref":"#/$defs/ArtifactKind"},"text":{"type":"string"}},"required":["kind","text"],"type":"object"};
const schema96 = {"enum":["powerio_ir","geo_layer","evidence"],"type":"string"};

function validate241(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate241.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate241.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!(((data === "powerio_ir") || (data === "geo_layer")) || (data === "evidence"))){
validate241.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema96.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate241.errors = vErrors;
return errors === 0;
}
validate241.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate240(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate240.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.kind === undefined) && (missing0 = "kind")) || ((data.text === undefined) && (missing0 = "text"))){
validate240.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!((key0 === "kind") || (key0 === "text"))){
validate240.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.kind !== undefined){
const _errs2 = errors;
if(!(validate241(data.kind, {instancePath:instancePath+"/kind",parentData:data,parentDataProperty:"kind",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate241.errors : vErrors.concat(validate241.errors);
errors = vErrors.length;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.text !== undefined){
const _errs3 = errors;
if(typeof data.text !== "string"){
validate240.errors = [{instancePath:instancePath+"/text",schemaPath:"#/properties/text/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate240.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate240.errors = vErrors;
return errors === 0;
}
validate240.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

const schema97 = {"additionalProperties":false,"description":"A document contains references, while the bundle owns deduplicated artifacts.","properties":{"active_goal":{"type":["string","null"]},"applied_state":{"type":["string","null"]},"base_input":{"description":"Original network input, retained independently of the starting operating point.","type":["string","null"]},"decisions":{"additionalProperties":{"$ref":"#/$defs/DecisionRecord"},"type":"object"},"display":{"anyOf":[{"$ref":"#/$defs/DisplayContext"},{"type":"null"}]},"experiment_order":{"items":{"type":"string"},"type":"array"},"experiments":{"additionalProperties":{"$ref":"#/$defs/ExperimentRecord"},"type":"object"},"goals":{"additionalProperties":{"$ref":"#/$defs/GoalRevision"},"type":"object"},"id":{"type":"string"},"inspected_state":{"type":["string","null"]},"model_details":{"anyOf":[{"$ref":"#/$defs/ModelDetails"},{"type":"null"}]},"recommended_state":{"type":["string","null"]},"revision":{"format":"uint64","minimum":0,"type":"integer"},"schema":{"type":"string"},"states":{"additionalProperties":{"$ref":"#/$defs/StateNode"},"type":"object"},"title":{"type":"string"},"version":{"format":"uint32","minimum":0,"type":"integer"}},"required":["schema","version","id","title","revision","goals","states","experiments","experiment_order","decisions"],"type":"object"};
const func1 = Object.prototype.hasOwnProperty;
const schema98 = {"additionalProperties":false,"properties":{"choice":{"$ref":"#/$defs/DecisionKind"},"evidence":{"items":{"type":"string"},"type":"array"},"experiment":{"type":"string"},"rationale":{"type":"string"},"state":{"type":["string","null"]}},"required":["experiment","choice","rationale","evidence"],"type":"object"};
const schema99 = {"enum":["retain","reject","recommend","apply"],"type":"string"};

function validate246(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate246.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate246.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((((data === "retain") || (data === "reject")) || (data === "recommend")) || (data === "apply"))){
validate246.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema99.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate246.errors = vErrors;
return errors === 0;
}
validate246.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate245(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate245.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((data.experiment === undefined) && (missing0 = "experiment")) || ((data.choice === undefined) && (missing0 = "choice"))) || ((data.rationale === undefined) && (missing0 = "rationale"))) || ((data.evidence === undefined) && (missing0 = "evidence"))){
validate245.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(((((key0 === "choice") || (key0 === "evidence")) || (key0 === "experiment")) || (key0 === "rationale")) || (key0 === "state"))){
validate245.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.choice !== undefined){
const _errs2 = errors;
if(!(validate246(data.choice, {instancePath:instancePath+"/choice",parentData:data,parentDataProperty:"choice",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate246.errors : vErrors.concat(validate246.errors);
errors = vErrors.length;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.evidence !== undefined){
let data1 = data.evidence;
const _errs3 = errors;
if(errors === _errs3){
if(Array.isArray(data1)){
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
const _errs5 = errors;
if(typeof data1[i0] !== "string"){
validate245.errors = [{instancePath:instancePath+"/evidence/" + i0,schemaPath:"#/properties/evidence/items/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid1 = _errs5 === errors;
if(!valid1){
break;
}
}
}
else {
validate245.errors = [{instancePath:instancePath+"/evidence",schemaPath:"#/properties/evidence/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.experiment !== undefined){
const _errs7 = errors;
if(typeof data.experiment !== "string"){
validate245.errors = [{instancePath:instancePath+"/experiment",schemaPath:"#/properties/experiment/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rationale !== undefined){
const _errs9 = errors;
if(typeof data.rationale !== "string"){
validate245.errors = [{instancePath:instancePath+"/rationale",schemaPath:"#/properties/rationale/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.state !== undefined){
let data5 = data.state;
const _errs11 = errors;
if((typeof data5 !== "string") && (data5 !== null)){
validate245.errors = [{instancePath:instancePath+"/state",schemaPath:"#/properties/state/type",keyword:"type",params:{type: schema98.properties.state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
else {
validate245.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate245.errors = vErrors;
return errors === 0;
}
validate245.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

const schema100 = {"additionalProperties":false,"properties":{"camera":{"anyOf":[{"$ref":"#/$defs/Camera"},{"type":"null"}]},"case_id":{"type":"string"},"diagram_camera":{"anyOf":[{"$ref":"#/$defs/DiagramCamera"},{"type":"null"}],"default":null},"geography":{"description":"Content-addressed PowerIO GeoLayer used for display, independent of physics.","type":"string"},"layers":{"default":[],"items":{"type":"string"},"type":"array"}},"required":["case_id","geography"],"type":"object"};
const schema101 = {"additionalProperties":false,"properties":{"bearing":{"format":"double","type":"number"},"center":{"items":{"format":"double","type":"number"},"maxItems":2,"minItems":2,"type":"array"},"pitch":{"format":"double","type":"number"},"zoom":{"format":"double","type":"number"}},"required":["center","zoom","bearing","pitch"],"type":"object"};

function validate250(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate250.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((data.center === undefined) && (missing0 = "center")) || ((data.zoom === undefined) && (missing0 = "zoom"))) || ((data.bearing === undefined) && (missing0 = "bearing"))) || ((data.pitch === undefined) && (missing0 = "pitch"))){
validate250.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!((((key0 === "bearing") || (key0 === "center")) || (key0 === "pitch")) || (key0 === "zoom"))){
validate250.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.bearing !== undefined){
let data0 = data.bearing;
const _errs2 = errors;
if(errors === _errs2){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate250.errors = [{instancePath:instancePath+"/bearing",schemaPath:"#/properties/bearing/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.center !== undefined){
let data1 = data.center;
const _errs4 = errors;
if(errors === _errs4){
if(Array.isArray(data1)){
if(data1.length > 2){
validate250.errors = [{instancePath:instancePath+"/center",schemaPath:"#/properties/center/maxItems",keyword:"maxItems",params:{limit: 2},message:"must NOT have more than 2 items"}];
return false;
}
else {
if(data1.length < 2){
validate250.errors = [{instancePath:instancePath+"/center",schemaPath:"#/properties/center/minItems",keyword:"minItems",params:{limit: 2},message:"must NOT have fewer than 2 items"}];
return false;
}
else {
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
let data2 = data1[i0];
const _errs6 = errors;
if(errors === _errs6){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate250.errors = [{instancePath:instancePath+"/center/" + i0,schemaPath:"#/properties/center/items/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid1 = _errs6 === errors;
if(!valid1){
break;
}
}
}
}
}
else {
validate250.errors = [{instancePath:instancePath+"/center",schemaPath:"#/properties/center/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.pitch !== undefined){
let data3 = data.pitch;
const _errs8 = errors;
if(errors === _errs8){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate250.errors = [{instancePath:instancePath+"/pitch",schemaPath:"#/properties/pitch/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.zoom !== undefined){
let data4 = data.zoom;
const _errs10 = errors;
if(errors === _errs10){
if(!((typeof data4 == "number") && (isFinite(data4)))){
validate250.errors = [{instancePath:instancePath+"/zoom",schemaPath:"#/properties/zoom/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
else {
validate250.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate250.errors = vErrors;
return errors === 0;
}
validate250.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

const schema102 = {"additionalProperties":false,"properties":{"center":{"items":{"format":"double","type":"number"},"maxItems":2,"minItems":2,"type":"array"},"scale":{"format":"double","type":"number"}},"required":["center","scale"],"type":"object"};

function validate252(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate252.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.center === undefined) && (missing0 = "center")) || ((data.scale === undefined) && (missing0 = "scale"))){
validate252.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!((key0 === "center") || (key0 === "scale"))){
validate252.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.center !== undefined){
let data0 = data.center;
const _errs2 = errors;
if(errors === _errs2){
if(Array.isArray(data0)){
if(data0.length > 2){
validate252.errors = [{instancePath:instancePath+"/center",schemaPath:"#/properties/center/maxItems",keyword:"maxItems",params:{limit: 2},message:"must NOT have more than 2 items"}];
return false;
}
else {
if(data0.length < 2){
validate252.errors = [{instancePath:instancePath+"/center",schemaPath:"#/properties/center/minItems",keyword:"minItems",params:{limit: 2},message:"must NOT have fewer than 2 items"}];
return false;
}
else {
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
let data1 = data0[i0];
const _errs4 = errors;
if(errors === _errs4){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate252.errors = [{instancePath:instancePath+"/center/" + i0,schemaPath:"#/properties/center/items/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid1 = _errs4 === errors;
if(!valid1){
break;
}
}
}
}
}
else {
validate252.errors = [{instancePath:instancePath+"/center",schemaPath:"#/properties/center/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.scale !== undefined){
let data2 = data.scale;
const _errs6 = errors;
if(errors === _errs6){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate252.errors = [{instancePath:instancePath+"/scale",schemaPath:"#/properties/scale/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate252.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate252.errors = vErrors;
return errors === 0;
}
validate252.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};


function validate249(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate249.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.case_id === undefined) && (missing0 = "case_id")) || ((data.geography === undefined) && (missing0 = "geography"))){
validate249.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(((((key0 === "camera") || (key0 === "case_id")) || (key0 === "diagram_camera")) || (key0 === "geography")) || (key0 === "layers"))){
validate249.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.camera !== undefined){
let data0 = data.camera;
const _errs2 = errors;
const _errs3 = errors;
let valid1 = false;
const _errs4 = errors;
if(!(validate250(data0, {instancePath:instancePath+"/camera",parentData:data,parentDataProperty:"camera",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate250.errors : vErrors.concat(validate250.errors);
errors = vErrors.length;
}
var _valid0 = _errs4 === errors;
valid1 = valid1 || _valid0;
const _errs5 = errors;
if(data0 !== null){
const err0 = {instancePath:instancePath+"/camera",schemaPath:"#/properties/camera/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs5 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/camera",schemaPath:"#/properties/camera/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate249.errors = vErrors;
return false;
}
else {
errors = _errs3;
if(vErrors !== null){
if(_errs3){
vErrors.length = _errs3;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.case_id !== undefined){
const _errs7 = errors;
if(typeof data.case_id !== "string"){
validate249.errors = [{instancePath:instancePath+"/case_id",schemaPath:"#/properties/case_id/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.diagram_camera !== undefined){
let data2 = data.diagram_camera;
const _errs9 = errors;
const _errs10 = errors;
let valid2 = false;
const _errs11 = errors;
if(!(validate252(data2, {instancePath:instancePath+"/diagram_camera",parentData:data,parentDataProperty:"diagram_camera",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate252.errors : vErrors.concat(validate252.errors);
errors = vErrors.length;
}
var _valid1 = _errs11 === errors;
valid2 = valid2 || _valid1;
const _errs12 = errors;
if(data2 !== null){
const err2 = {instancePath:instancePath+"/diagram_camera",schemaPath:"#/properties/diagram_camera/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs12 === errors;
valid2 = valid2 || _valid1;
if(!valid2){
const err3 = {instancePath:instancePath+"/diagram_camera",schemaPath:"#/properties/diagram_camera/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate249.errors = vErrors;
return false;
}
else {
errors = _errs10;
if(vErrors !== null){
if(_errs10){
vErrors.length = _errs10;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.geography !== undefined){
const _errs14 = errors;
if(typeof data.geography !== "string"){
validate249.errors = [{instancePath:instancePath+"/geography",schemaPath:"#/properties/geography/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs14 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.layers !== undefined){
let data4 = data.layers;
const _errs16 = errors;
if(errors === _errs16){
if(Array.isArray(data4)){
var valid3 = true;
const len0 = data4.length;
for(let i0=0; i0<len0; i0++){
const _errs18 = errors;
if(typeof data4[i0] !== "string"){
validate249.errors = [{instancePath:instancePath+"/layers/" + i0,schemaPath:"#/properties/layers/items/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid3 = _errs18 === errors;
if(!valid3){
break;
}
}
}
else {
validate249.errors = [{instancePath:instancePath+"/layers",schemaPath:"#/properties/layers/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs16 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
else {
validate249.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate249.errors = vErrors;
return errors === 0;
}
validate249.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

const schema103 = {"additionalProperties":false,"description":"Stated rationales and evidence references describe observable agent actions.","properties":{"assessed_recommendation":{"type":["string","null"]},"evidence":{"items":{"type":"string"},"type":"array"},"goal":{"type":["string","null"]},"kind":{"$ref":"#/$defs/ExperimentKind"},"rationale":{"type":"string"},"result_states":{"items":{"type":"string"},"type":"array"},"solve_count":{"format":"uint","minimum":0,"type":"integer"},"start_state":{"description":"Historical imports can lack recoverable electrical state and goal data.","type":["string","null"]},"termination":{"type":"string"},"trials":{"items":{"$ref":"#/$defs/TrialRecord"},"type":"array"}},"required":["kind","rationale","evidence","trials","result_states","solve_count","termination"],"type":"object"};
const schema104 = {"enum":["inspection","sensitivity","planning","counterfactual","challenge","historical_import"],"type":"string"};

function validate256(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate256.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate256.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((((((data === "inspection") || (data === "sensitivity")) || (data === "planning")) || (data === "counterfactual")) || (data === "challenge")) || (data === "historical_import"))){
validate256.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema104.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate256.errors = vErrors;
return errors === 0;
}
validate256.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema105 = {"additionalProperties":false,"properties":{"accepted":{"type":"boolean"},"changes":{"items":{"format":"double","type":"number"},"type":"array"},"evidence":{"items":{"type":"string"},"type":"array"},"exact_value":{"format":"double","type":["number","null"]},"failure":{"type":["string","null"]},"predicted_value":{"format":"double","type":["number","null"]},"state":{"type":["string","null"]}},"required":["changes","accepted","evidence"],"type":"object"};

function validate258(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate258.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.changes === undefined) && (missing0 = "changes")) || ((data.accepted === undefined) && (missing0 = "accepted"))) || ((data.evidence === undefined) && (missing0 = "evidence"))){
validate258.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(((((((key0 === "accepted") || (key0 === "changes")) || (key0 === "evidence")) || (key0 === "exact_value")) || (key0 === "failure")) || (key0 === "predicted_value")) || (key0 === "state"))){
validate258.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.accepted !== undefined){
const _errs2 = errors;
if(typeof data.accepted !== "boolean"){
validate258.errors = [{instancePath:instancePath+"/accepted",schemaPath:"#/properties/accepted/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.changes !== undefined){
let data1 = data.changes;
const _errs4 = errors;
if(errors === _errs4){
if(Array.isArray(data1)){
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
let data2 = data1[i0];
const _errs6 = errors;
if(errors === _errs6){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate258.errors = [{instancePath:instancePath+"/changes/" + i0,schemaPath:"#/properties/changes/items/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid1 = _errs6 === errors;
if(!valid1){
break;
}
}
}
else {
validate258.errors = [{instancePath:instancePath+"/changes",schemaPath:"#/properties/changes/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.evidence !== undefined){
let data3 = data.evidence;
const _errs8 = errors;
if(errors === _errs8){
if(Array.isArray(data3)){
var valid2 = true;
const len1 = data3.length;
for(let i1=0; i1<len1; i1++){
const _errs10 = errors;
if(typeof data3[i1] !== "string"){
validate258.errors = [{instancePath:instancePath+"/evidence/" + i1,schemaPath:"#/properties/evidence/items/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid2 = _errs10 === errors;
if(!valid2){
break;
}
}
}
else {
validate258.errors = [{instancePath:instancePath+"/evidence",schemaPath:"#/properties/evidence/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.exact_value !== undefined){
let data5 = data.exact_value;
const _errs12 = errors;
if((!((typeof data5 == "number") && (isFinite(data5)))) && (data5 !== null)){
validate258.errors = [{instancePath:instancePath+"/exact_value",schemaPath:"#/properties/exact_value/type",keyword:"type",params:{type: schema105.properties.exact_value.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.failure !== undefined){
let data6 = data.failure;
const _errs14 = errors;
if((typeof data6 !== "string") && (data6 !== null)){
validate258.errors = [{instancePath:instancePath+"/failure",schemaPath:"#/properties/failure/type",keyword:"type",params:{type: schema105.properties.failure.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs14 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.predicted_value !== undefined){
let data7 = data.predicted_value;
const _errs16 = errors;
if((!((typeof data7 == "number") && (isFinite(data7)))) && (data7 !== null)){
validate258.errors = [{instancePath:instancePath+"/predicted_value",schemaPath:"#/properties/predicted_value/type",keyword:"type",params:{type: schema105.properties.predicted_value.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs16 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.state !== undefined){
let data8 = data.state;
const _errs18 = errors;
if((typeof data8 !== "string") && (data8 !== null)){
validate258.errors = [{instancePath:instancePath+"/state",schemaPath:"#/properties/state/type",keyword:"type",params:{type: schema105.properties.state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
else {
validate258.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate258.errors = vErrors;
return errors === 0;
}
validate258.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};


function validate255(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate255.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.kind === undefined) && (missing0 = "kind")) || ((data.rationale === undefined) && (missing0 = "rationale"))) || ((data.evidence === undefined) && (missing0 = "evidence"))) || ((data.trials === undefined) && (missing0 = "trials"))) || ((data.result_states === undefined) && (missing0 = "result_states"))) || ((data.solve_count === undefined) && (missing0 = "solve_count"))) || ((data.termination === undefined) && (missing0 = "termination"))){
validate255.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(func1.call(schema103.properties, key0))){
validate255.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.assessed_recommendation !== undefined){
let data0 = data.assessed_recommendation;
const _errs2 = errors;
if((typeof data0 !== "string") && (data0 !== null)){
validate255.errors = [{instancePath:instancePath+"/assessed_recommendation",schemaPath:"#/properties/assessed_recommendation/type",keyword:"type",params:{type: schema103.properties.assessed_recommendation.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.evidence !== undefined){
let data1 = data.evidence;
const _errs4 = errors;
if(errors === _errs4){
if(Array.isArray(data1)){
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
const _errs6 = errors;
if(typeof data1[i0] !== "string"){
validate255.errors = [{instancePath:instancePath+"/evidence/" + i0,schemaPath:"#/properties/evidence/items/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid1 = _errs6 === errors;
if(!valid1){
break;
}
}
}
else {
validate255.errors = [{instancePath:instancePath+"/evidence",schemaPath:"#/properties/evidence/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.goal !== undefined){
let data3 = data.goal;
const _errs8 = errors;
if((typeof data3 !== "string") && (data3 !== null)){
validate255.errors = [{instancePath:instancePath+"/goal",schemaPath:"#/properties/goal/type",keyword:"type",params:{type: schema103.properties.goal.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.kind !== undefined){
const _errs10 = errors;
if(!(validate256(data.kind, {instancePath:instancePath+"/kind",parentData:data,parentDataProperty:"kind",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate256.errors : vErrors.concat(validate256.errors);
errors = vErrors.length;
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rationale !== undefined){
const _errs11 = errors;
if(typeof data.rationale !== "string"){
validate255.errors = [{instancePath:instancePath+"/rationale",schemaPath:"#/properties/rationale/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.result_states !== undefined){
let data6 = data.result_states;
const _errs13 = errors;
if(errors === _errs13){
if(Array.isArray(data6)){
var valid2 = true;
const len1 = data6.length;
for(let i1=0; i1<len1; i1++){
const _errs15 = errors;
if(typeof data6[i1] !== "string"){
validate255.errors = [{instancePath:instancePath+"/result_states/" + i1,schemaPath:"#/properties/result_states/items/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid2 = _errs15 === errors;
if(!valid2){
break;
}
}
}
else {
validate255.errors = [{instancePath:instancePath+"/result_states",schemaPath:"#/properties/result_states/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.solve_count !== undefined){
let data8 = data.solve_count;
const _errs17 = errors;
if(!(((typeof data8 == "number") && (!(data8 % 1) && !isNaN(data8))) && (isFinite(data8)))){
validate255.errors = [{instancePath:instancePath+"/solve_count",schemaPath:"#/properties/solve_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs17){
if((typeof data8 == "number") && (isFinite(data8))){
if(data8 < 0 || isNaN(data8)){
validate255.errors = [{instancePath:instancePath+"/solve_count",schemaPath:"#/properties/solve_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs17 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.start_state !== undefined){
let data9 = data.start_state;
const _errs19 = errors;
if((typeof data9 !== "string") && (data9 !== null)){
validate255.errors = [{instancePath:instancePath+"/start_state",schemaPath:"#/properties/start_state/type",keyword:"type",params:{type: schema103.properties.start_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs19 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.termination !== undefined){
const _errs21 = errors;
if(typeof data.termination !== "string"){
validate255.errors = [{instancePath:instancePath+"/termination",schemaPath:"#/properties/termination/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.trials !== undefined){
let data11 = data.trials;
const _errs23 = errors;
if(errors === _errs23){
if(Array.isArray(data11)){
var valid3 = true;
const len2 = data11.length;
for(let i2=0; i2<len2; i2++){
const _errs25 = errors;
if(!(validate258(data11[i2], {instancePath:instancePath+"/trials/" + i2,parentData:data11,parentDataProperty:i2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate258.errors : vErrors.concat(validate258.errors);
errors = vErrors.length;
}
var valid3 = _errs25 === errors;
if(!valid3){
break;
}
}
}
else {
validate255.errors = [{instancePath:instancePath+"/trials",schemaPath:"#/properties/trials/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs23 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate255.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate255.errors = vErrors;
return errors === 0;
}
validate255.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

const schema106 = {"additionalProperties":false,"description":"A goal revision retains its own anchor and interpretation.","properties":{"anchor_state":{"type":"string"},"decisions":{"$ref":"#/$defs/DecisionSpace"},"interpretation":{"type":"string"},"objective":{"$ref":"#/$defs/StudyObjective"},"parent":{"type":["string","null"]},"request":{"type":"string"},"success_value":{"format":"double","type":["number","null"]}},"required":["anchor_state","request","interpretation","objective","decisions"],"type":"object"};
const schema107 = {"additionalProperties":false,"description":"The feasible set, distinct from both the inner problem and the outer goal.","properties":{"demand":{"anyOf":[{"$ref":"#/$defs/DemandConstraint"},{"type":"null"}]},"max_changed_elements":{"format":"uint","maximum":65536,"minimum":1,"type":"integer"},"total_budget":{"format":"double","minimum":0,"type":"number"},"variables":{"items":{"$ref":"#/$defs/DecisionVariable"},"maxItems":65536,"minItems":1,"type":"array"}},"required":["variables","total_budget","max_changed_elements"],"type":"object"};
const schema108 = {"oneOf":[{"additionalProperties":false,"description":"Allocate this additional MW across the declared demand decisions.","properties":{"increase_mw":{"format":"double","type":"number"},"kind":{"const":"placement","type":"string"}},"required":["kind","increase_mw"],"type":"object"},{"additionalProperties":false,"description":"Preserve the existing total MW across the declared demand decisions.","properties":{"kind":{"const":"redistribution","type":"string"}},"required":["kind"],"type":"object"}]};

function validate263(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate263.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(errors === _errs1){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.kind === undefined) && (missing0 = "kind")) || ((data.increase_mw === undefined) && (missing0 = "increase_mw"))){
const err0 = {instancePath,schemaPath:"#/oneOf/0/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
else {
const _errs3 = errors;
for(const key0 in data){
if(!((key0 === "increase_mw") || (key0 === "kind"))){
const err1 = {instancePath,schemaPath:"#/oneOf/0/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
break;
}
}
if(_errs3 === errors){
if(data.increase_mw !== undefined){
let data0 = data.increase_mw;
const _errs4 = errors;
if(errors === _errs4){
if(!((typeof data0 == "number") && (isFinite(data0)))){
const err2 = {instancePath:instancePath+"/increase_mw",schemaPath:"#/oneOf/0/properties/increase_mw/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
}
var valid1 = _errs4 === errors;
}
else {
var valid1 = true;
}
if(valid1){
if(data.kind !== undefined){
let data1 = data.kind;
const _errs6 = errors;
if(typeof data1 !== "string"){
const err3 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/0/properties/kind/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
if("placement" !== data1){
const err4 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/0/properties/kind/const",keyword:"const",params:{allowedValue: "placement"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
var valid1 = _errs6 === errors;
}
else {
var valid1 = true;
}
}
}
}
}
else {
const err5 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
var props0 = true;
}
const _errs8 = errors;
if(errors === _errs8){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing1;
if((data.kind === undefined) && (missing1 = "kind")){
const err6 = {instancePath,schemaPath:"#/oneOf/1/required",keyword:"required",params:{missingProperty: missing1},message:"must have required property '"+missing1+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
else {
const _errs10 = errors;
for(const key1 in data){
if(!(key1 === "kind")){
const err7 = {instancePath,schemaPath:"#/oneOf/1/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
break;
}
}
if(_errs10 === errors){
if(data.kind !== undefined){
let data2 = data.kind;
if(typeof data2 !== "string"){
const err8 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/1/properties/kind/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
if("redistribution" !== data2){
const err9 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/1/properties/kind/const",keyword:"const",params:{allowedValue: "redistribution"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
}
}
}
else {
const err10 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
var _valid0 = _errs8 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
if(props0 !== true){
props0 = true;
}
}
}
if(!valid0){
const err11 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
validate263.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate263.errors = vErrors;
evaluated0.props = props0;
return errors === 0;
}
validate263.evaluated = {"dynamicProps":true,"dynamicItems":false};

const schema109 = {"additionalProperties":false,"description":"Bounds and increments apply to changes from the goal's anchor state in a Study.","properties":{"budget_weight":{"description":"Budget consumption per absolute unit of intervention.","format":"double","type":"number"},"element":{"$ref":"#/$defs/ElementKey"},"id":{"maxLength":128,"minLength":1,"type":"string"},"increment":{"format":"double","type":"number"},"intervention":{"$ref":"#/$defs/Intervention"},"lower":{"format":"double","type":"number"},"upper":{"format":"double","type":"number"}},"required":["id","element","intervention","lower","upper","increment","budget_weight"],"type":"object"};
const schema110 = {"oneOf":[{"type":"integer"},{"type":"string"}]};

function validate266(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate266.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(!(((typeof data == "number") && (!(data % 1) && !isNaN(data))) && (isFinite(data)))){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err1 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
}
if(!valid0){
const err2 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
validate266.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate266.errors = vErrors;
return errors === 0;
}
validate266.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema111 = {"description":"A numerical intervention supported by the existing solvers.","enum":["branch_rating","active_demand"],"type":"string"};

function validate268(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate268.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate268.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "branch_rating") || (data === "active_demand"))){
validate268.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema111.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate268.errors = vErrors;
return errors === 0;
}
validate268.evaluated = {"dynamicProps":false,"dynamicItems":false};

const func3 = (text => { let size = 0; for (const character of text) size++; return size; });

function validate265(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate265.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.id === undefined) && (missing0 = "id")) || ((data.element === undefined) && (missing0 = "element"))) || ((data.intervention === undefined) && (missing0 = "intervention"))) || ((data.lower === undefined) && (missing0 = "lower"))) || ((data.upper === undefined) && (missing0 = "upper"))) || ((data.increment === undefined) && (missing0 = "increment"))) || ((data.budget_weight === undefined) && (missing0 = "budget_weight"))){
validate265.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(((((((key0 === "budget_weight") || (key0 === "element")) || (key0 === "id")) || (key0 === "increment")) || (key0 === "intervention")) || (key0 === "lower")) || (key0 === "upper"))){
validate265.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.budget_weight !== undefined){
let data0 = data.budget_weight;
const _errs2 = errors;
if(errors === _errs2){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate265.errors = [{instancePath:instancePath+"/budget_weight",schemaPath:"#/properties/budget_weight/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.element !== undefined){
const _errs4 = errors;
if(!(validate266(data.element, {instancePath:instancePath+"/element",parentData:data,parentDataProperty:"element",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate266.errors : vErrors.concat(validate266.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.id !== undefined){
let data2 = data.id;
const _errs5 = errors;
if(errors === _errs5){
if(typeof data2 === "string"){
if(func3(data2) > 128){
validate265.errors = [{instancePath:instancePath+"/id",schemaPath:"#/properties/id/maxLength",keyword:"maxLength",params:{limit: 128},message:"must NOT have more than 128 characters"}];
return false;
}
else {
if(func3(data2) < 1){
validate265.errors = [{instancePath:instancePath+"/id",schemaPath:"#/properties/id/minLength",keyword:"minLength",params:{limit: 1},message:"must NOT have fewer than 1 characters"}];
return false;
}
}
}
else {
validate265.errors = [{instancePath:instancePath+"/id",schemaPath:"#/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.increment !== undefined){
let data3 = data.increment;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate265.errors = [{instancePath:instancePath+"/increment",schemaPath:"#/properties/increment/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.intervention !== undefined){
const _errs9 = errors;
if(!(validate268(data.intervention, {instancePath:instancePath+"/intervention",parentData:data,parentDataProperty:"intervention",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate268.errors : vErrors.concat(validate268.errors);
errors = vErrors.length;
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lower !== undefined){
let data5 = data.lower;
const _errs10 = errors;
if(errors === _errs10){
if(!((typeof data5 == "number") && (isFinite(data5)))){
validate265.errors = [{instancePath:instancePath+"/lower",schemaPath:"#/properties/lower/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.upper !== undefined){
let data6 = data.upper;
const _errs12 = errors;
if(errors === _errs12){
if(!((typeof data6 == "number") && (isFinite(data6)))){
validate265.errors = [{instancePath:instancePath+"/upper",schemaPath:"#/properties/upper/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
else {
validate265.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate265.errors = vErrors;
return errors === 0;
}
validate265.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};


function validate262(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate262.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.variables === undefined) && (missing0 = "variables")) || ((data.total_budget === undefined) && (missing0 = "total_budget"))) || ((data.max_changed_elements === undefined) && (missing0 = "max_changed_elements"))){
validate262.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!((((key0 === "demand") || (key0 === "max_changed_elements")) || (key0 === "total_budget")) || (key0 === "variables"))){
validate262.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.demand !== undefined){
let data0 = data.demand;
const _errs2 = errors;
const _errs3 = errors;
let valid1 = false;
const _errs4 = errors;
if(!(validate263(data0, {instancePath:instancePath+"/demand",parentData:data,parentDataProperty:"demand",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate263.errors : vErrors.concat(validate263.errors);
errors = vErrors.length;
}
var _valid0 = _errs4 === errors;
valid1 = valid1 || _valid0;
const _errs5 = errors;
if(data0 !== null){
const err0 = {instancePath:instancePath+"/demand",schemaPath:"#/properties/demand/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs5 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/demand",schemaPath:"#/properties/demand/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate262.errors = vErrors;
return false;
}
else {
errors = _errs3;
if(vErrors !== null){
if(_errs3){
vErrors.length = _errs3;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.max_changed_elements !== undefined){
let data1 = data.max_changed_elements;
const _errs7 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate262.errors = [{instancePath:instancePath+"/max_changed_elements",schemaPath:"#/properties/max_changed_elements/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs7){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 > 65536 || isNaN(data1)){
validate262.errors = [{instancePath:instancePath+"/max_changed_elements",schemaPath:"#/properties/max_changed_elements/maximum",keyword:"maximum",params:{comparison: "<=", limit: 65536},message:"must be <= 65536"}];
return false;
}
else {
if(data1 < 1 || isNaN(data1)){
validate262.errors = [{instancePath:instancePath+"/max_changed_elements",schemaPath:"#/properties/max_changed_elements/minimum",keyword:"minimum",params:{comparison: ">=", limit: 1},message:"must be >= 1"}];
return false;
}
}
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.total_budget !== undefined){
let data2 = data.total_budget;
const _errs9 = errors;
if(errors === _errs9){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate262.errors = [{instancePath:instancePath+"/total_budget",schemaPath:"#/properties/total_budget/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
else {
validate262.errors = [{instancePath:instancePath+"/total_budget",schemaPath:"#/properties/total_budget/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.variables !== undefined){
let data3 = data.variables;
const _errs11 = errors;
if(errors === _errs11){
if(Array.isArray(data3)){
if(data3.length > 65536){
validate262.errors = [{instancePath:instancePath+"/variables",schemaPath:"#/properties/variables/maxItems",keyword:"maxItems",params:{limit: 65536},message:"must NOT have more than 65536 items"}];
return false;
}
else {
if(data3.length < 1){
validate262.errors = [{instancePath:instancePath+"/variables",schemaPath:"#/properties/variables/minItems",keyword:"minItems",params:{limit: 1},message:"must NOT have fewer than 1 items"}];
return false;
}
else {
var valid2 = true;
const len0 = data3.length;
for(let i0=0; i0<len0; i0++){
const _errs13 = errors;
if(!(validate265(data3[i0], {instancePath:instancePath+"/variables/" + i0,parentData:data3,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate265.errors : vErrors.concat(validate265.errors);
errors = vErrors.length;
}
var valid2 = _errs13 === errors;
if(!valid2){
break;
}
}
}
}
}
else {
validate262.errors = [{instancePath:instancePath+"/variables",schemaPath:"#/properties/variables/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
else {
validate262.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate262.errors = vErrors;
return errors === 0;
}
validate262.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

const schema112 = {"description":"A scalar goal over the exact operating point and explicit interventions.","oneOf":[{"additionalProperties":false,"properties":{"kind":{"const":"weighted_observable","type":"string"},"operand":{"$ref":"#/$defs/Operand"},"weights":{"items":{"$ref":"#/$defs/ObservableWeight"},"maxItems":65536,"minItems":1,"type":"array"}},"required":["kind","operand","weights"],"type":"object"},{"additionalProperties":false,"properties":{"kind":{"const":"sum","type":"string"},"terms":{"items":{"$ref":"#/$defs/StudyObjective"},"maxItems":65536,"minItems":1,"type":"array"}},"required":["kind","terms"],"type":"object"},{"additionalProperties":false,"properties":{"expression":{"$ref":"#/$defs/StudyObjective"},"factor":{"format":"double","type":"number"},"kind":{"const":"scale","type":"string"}},"required":["kind","factor","expression"],"type":"object"},{"additionalProperties":false,"properties":{"expression":{"$ref":"#/$defs/StudyObjective"},"kind":{"const":"squared_target","type":"string"},"target":{"format":"double","type":"number"}},"required":["kind","target","expression"],"type":"object"},{"additionalProperties":false,"properties":{"decision":{"type":"string"},"kind":{"const":"intervention_penalty","type":"string"},"linear":{"format":"double","type":"number"},"quadratic":{"format":"double","type":"number"}},"required":["kind","decision","linear","quadratic"],"type":"object"}]};
const schema113 = {"additionalProperties":false,"description":"A weighted observable in the units reported by the selected formulation.","properties":{"element":{"$ref":"#/$defs/ElementKey"},"weight":{"format":"double","type":"number"}},"required":["element","weight"],"type":"object"};

function validate274(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate274.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.element === undefined) && (missing0 = "element")) || ((data.weight === undefined) && (missing0 = "weight"))){
validate274.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!((key0 === "element") || (key0 === "weight"))){
validate274.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.element !== undefined){
const _errs2 = errors;
if(!(validate266(data.element, {instancePath:instancePath+"/element",parentData:data,parentDataProperty:"element",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate266.errors : vErrors.concat(validate266.errors);
errors = vErrors.length;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.weight !== undefined){
let data1 = data.weight;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate274.errors = [{instancePath:instancePath+"/weight",schemaPath:"#/properties/weight/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate274.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate274.errors = vErrors;
return errors === 0;
}
validate274.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

const wrapper0 = {validate: validate272};

function validate272(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate272.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(errors === _errs1){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.kind === undefined) && (missing0 = "kind")) || ((data.operand === undefined) && (missing0 = "operand"))) || ((data.weights === undefined) && (missing0 = "weights"))){
const err0 = {instancePath,schemaPath:"#/oneOf/0/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
else {
const _errs3 = errors;
for(const key0 in data){
if(!(((key0 === "kind") || (key0 === "operand")) || (key0 === "weights"))){
const err1 = {instancePath,schemaPath:"#/oneOf/0/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
break;
}
}
if(_errs3 === errors){
if(data.kind !== undefined){
let data0 = data.kind;
const _errs4 = errors;
if(typeof data0 !== "string"){
const err2 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/0/properties/kind/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("weighted_observable" !== data0){
const err3 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/0/properties/kind/const",keyword:"const",params:{allowedValue: "weighted_observable"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var valid1 = _errs4 === errors;
}
else {
var valid1 = true;
}
if(valid1){
if(data.operand !== undefined){
const _errs6 = errors;
if(!(validate88(data.operand, {instancePath:instancePath+"/operand",parentData:data,parentDataProperty:"operand",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate88.errors : vErrors.concat(validate88.errors);
errors = vErrors.length;
}
var valid1 = _errs6 === errors;
}
else {
var valid1 = true;
}
if(valid1){
if(data.weights !== undefined){
let data2 = data.weights;
const _errs7 = errors;
if(errors === _errs7){
if(Array.isArray(data2)){
if(data2.length > 65536){
const err4 = {instancePath:instancePath+"/weights",schemaPath:"#/oneOf/0/properties/weights/maxItems",keyword:"maxItems",params:{limit: 65536},message:"must NOT have more than 65536 items"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
else {
if(data2.length < 1){
const err5 = {instancePath:instancePath+"/weights",schemaPath:"#/oneOf/0/properties/weights/minItems",keyword:"minItems",params:{limit: 1},message:"must NOT have fewer than 1 items"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
else {
var valid2 = true;
const len0 = data2.length;
for(let i0=0; i0<len0; i0++){
const _errs9 = errors;
if(!(validate274(data2[i0], {instancePath:instancePath+"/weights/" + i0,parentData:data2,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate274.errors : vErrors.concat(validate274.errors);
errors = vErrors.length;
}
var valid2 = _errs9 === errors;
if(!valid2){
break;
}
}
}
}
}
else {
const err6 = {instancePath:instancePath+"/weights",schemaPath:"#/oneOf/0/properties/weights/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
}
var valid1 = _errs7 === errors;
}
else {
var valid1 = true;
}
}
}
}
}
}
else {
const err7 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
var props1 = true;
}
const _errs10 = errors;
if(errors === _errs10){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing1;
if(((data.kind === undefined) && (missing1 = "kind")) || ((data.terms === undefined) && (missing1 = "terms"))){
const err8 = {instancePath,schemaPath:"#/oneOf/1/required",keyword:"required",params:{missingProperty: missing1},message:"must have required property '"+missing1+"'"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
else {
const _errs12 = errors;
for(const key1 in data){
if(!((key1 === "kind") || (key1 === "terms"))){
const err9 = {instancePath,schemaPath:"#/oneOf/1/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
break;
}
}
if(_errs12 === errors){
if(data.kind !== undefined){
let data4 = data.kind;
const _errs13 = errors;
if(typeof data4 !== "string"){
const err10 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/1/properties/kind/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
if("sum" !== data4){
const err11 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/1/properties/kind/const",keyword:"const",params:{allowedValue: "sum"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
var valid3 = _errs13 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.terms !== undefined){
let data5 = data.terms;
const _errs15 = errors;
if(errors === _errs15){
if(Array.isArray(data5)){
if(data5.length > 65536){
const err12 = {instancePath:instancePath+"/terms",schemaPath:"#/oneOf/1/properties/terms/maxItems",keyword:"maxItems",params:{limit: 65536},message:"must NOT have more than 65536 items"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
else {
if(data5.length < 1){
const err13 = {instancePath:instancePath+"/terms",schemaPath:"#/oneOf/1/properties/terms/minItems",keyword:"minItems",params:{limit: 1},message:"must NOT have fewer than 1 items"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
else {
var valid4 = true;
const len1 = data5.length;
for(let i1=0; i1<len1; i1++){
const _errs17 = errors;
if(!(wrapper0.validate(data5[i1], {instancePath:instancePath+"/terms/" + i1,parentData:data5,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? wrapper0.validate.errors : vErrors.concat(wrapper0.validate.errors);
errors = vErrors.length;
}
var valid4 = _errs17 === errors;
if(!valid4){
break;
}
}
}
}
}
else {
const err14 = {instancePath:instancePath+"/terms",schemaPath:"#/oneOf/1/properties/terms/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
}
var valid3 = _errs15 === errors;
}
else {
var valid3 = true;
}
}
}
}
}
else {
const err15 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
}
var _valid0 = _errs10 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
if(props1 !== true){
props1 = true;
}
}
const _errs18 = errors;
if(errors === _errs18){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing2;
if((((data.kind === undefined) && (missing2 = "kind")) || ((data.factor === undefined) && (missing2 = "factor"))) || ((data.expression === undefined) && (missing2 = "expression"))){
const err16 = {instancePath,schemaPath:"#/oneOf/2/required",keyword:"required",params:{missingProperty: missing2},message:"must have required property '"+missing2+"'"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
else {
const _errs20 = errors;
for(const key2 in data){
if(!(((key2 === "expression") || (key2 === "factor")) || (key2 === "kind"))){
const err17 = {instancePath,schemaPath:"#/oneOf/2/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
break;
}
}
if(_errs20 === errors){
if(data.expression !== undefined){
const _errs21 = errors;
if(!(wrapper0.validate(data.expression, {instancePath:instancePath+"/expression",parentData:data,parentDataProperty:"expression",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? wrapper0.validate.errors : vErrors.concat(wrapper0.validate.errors);
errors = vErrors.length;
}
var valid5 = _errs21 === errors;
}
else {
var valid5 = true;
}
if(valid5){
if(data.factor !== undefined){
let data8 = data.factor;
const _errs22 = errors;
if(errors === _errs22){
if(!((typeof data8 == "number") && (isFinite(data8)))){
const err18 = {instancePath:instancePath+"/factor",schemaPath:"#/oneOf/2/properties/factor/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
}
var valid5 = _errs22 === errors;
}
else {
var valid5 = true;
}
if(valid5){
if(data.kind !== undefined){
let data9 = data.kind;
const _errs24 = errors;
if(typeof data9 !== "string"){
const err19 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/2/properties/kind/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
if("scale" !== data9){
const err20 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/2/properties/kind/const",keyword:"const",params:{allowedValue: "scale"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
var valid5 = _errs24 === errors;
}
else {
var valid5 = true;
}
}
}
}
}
}
else {
const err21 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
}
var _valid0 = _errs18 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
if(props1 !== true){
props1 = true;
}
}
const _errs26 = errors;
if(errors === _errs26){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing3;
if((((data.kind === undefined) && (missing3 = "kind")) || ((data.target === undefined) && (missing3 = "target"))) || ((data.expression === undefined) && (missing3 = "expression"))){
const err22 = {instancePath,schemaPath:"#/oneOf/3/required",keyword:"required",params:{missingProperty: missing3},message:"must have required property '"+missing3+"'"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
else {
const _errs28 = errors;
for(const key3 in data){
if(!(((key3 === "expression") || (key3 === "kind")) || (key3 === "target"))){
const err23 = {instancePath,schemaPath:"#/oneOf/3/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key3},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
break;
}
}
if(_errs28 === errors){
if(data.expression !== undefined){
const _errs29 = errors;
if(!(wrapper0.validate(data.expression, {instancePath:instancePath+"/expression",parentData:data,parentDataProperty:"expression",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? wrapper0.validate.errors : vErrors.concat(wrapper0.validate.errors);
errors = vErrors.length;
}
var valid6 = _errs29 === errors;
}
else {
var valid6 = true;
}
if(valid6){
if(data.kind !== undefined){
let data11 = data.kind;
const _errs30 = errors;
if(typeof data11 !== "string"){
const err24 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/3/properties/kind/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
if("squared_target" !== data11){
const err25 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/3/properties/kind/const",keyword:"const",params:{allowedValue: "squared_target"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
var valid6 = _errs30 === errors;
}
else {
var valid6 = true;
}
if(valid6){
if(data.target !== undefined){
let data12 = data.target;
const _errs32 = errors;
if(errors === _errs32){
if(!((typeof data12 == "number") && (isFinite(data12)))){
const err26 = {instancePath:instancePath+"/target",schemaPath:"#/oneOf/3/properties/target/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
}
var valid6 = _errs32 === errors;
}
else {
var valid6 = true;
}
}
}
}
}
}
else {
const err27 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
}
var _valid0 = _errs26 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
if(props1 !== true){
props1 = true;
}
}
const _errs34 = errors;
if(errors === _errs34){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing4;
if(((((data.kind === undefined) && (missing4 = "kind")) || ((data.decision === undefined) && (missing4 = "decision"))) || ((data.linear === undefined) && (missing4 = "linear"))) || ((data.quadratic === undefined) && (missing4 = "quadratic"))){
const err28 = {instancePath,schemaPath:"#/oneOf/4/required",keyword:"required",params:{missingProperty: missing4},message:"must have required property '"+missing4+"'"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
else {
const _errs36 = errors;
for(const key4 in data){
if(!((((key4 === "decision") || (key4 === "kind")) || (key4 === "linear")) || (key4 === "quadratic"))){
const err29 = {instancePath,schemaPath:"#/oneOf/4/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key4},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
break;
}
}
if(_errs36 === errors){
if(data.decision !== undefined){
const _errs37 = errors;
if(typeof data.decision !== "string"){
const err30 = {instancePath:instancePath+"/decision",schemaPath:"#/oneOf/4/properties/decision/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err30];
}
else {
vErrors.push(err30);
}
errors++;
}
var valid7 = _errs37 === errors;
}
else {
var valid7 = true;
}
if(valid7){
if(data.kind !== undefined){
let data14 = data.kind;
const _errs39 = errors;
if(typeof data14 !== "string"){
const err31 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/4/properties/kind/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err31];
}
else {
vErrors.push(err31);
}
errors++;
}
if("intervention_penalty" !== data14){
const err32 = {instancePath:instancePath+"/kind",schemaPath:"#/oneOf/4/properties/kind/const",keyword:"const",params:{allowedValue: "intervention_penalty"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err32];
}
else {
vErrors.push(err32);
}
errors++;
}
var valid7 = _errs39 === errors;
}
else {
var valid7 = true;
}
if(valid7){
if(data.linear !== undefined){
let data15 = data.linear;
const _errs41 = errors;
if(errors === _errs41){
if(!((typeof data15 == "number") && (isFinite(data15)))){
const err33 = {instancePath:instancePath+"/linear",schemaPath:"#/oneOf/4/properties/linear/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err33];
}
else {
vErrors.push(err33);
}
errors++;
}
}
var valid7 = _errs41 === errors;
}
else {
var valid7 = true;
}
if(valid7){
if(data.quadratic !== undefined){
let data16 = data.quadratic;
const _errs43 = errors;
if(errors === _errs43){
if(!((typeof data16 == "number") && (isFinite(data16)))){
const err34 = {instancePath:instancePath+"/quadratic",schemaPath:"#/oneOf/4/properties/quadratic/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err34];
}
else {
vErrors.push(err34);
}
errors++;
}
}
var valid7 = _errs43 === errors;
}
else {
var valid7 = true;
}
}
}
}
}
}
}
else {
const err35 = {instancePath,schemaPath:"#/oneOf/4/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err35];
}
else {
vErrors.push(err35);
}
errors++;
}
}
var _valid0 = _errs34 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 4];
}
else {
if(_valid0){
valid0 = true;
passing0 = 4;
if(props1 !== true){
props1 = true;
}
}
}
}
}
}
if(!valid0){
const err36 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err36];
}
else {
vErrors.push(err36);
}
errors++;
validate272.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate272.errors = vErrors;
evaluated0.props = props1;
return errors === 0;
}
validate272.evaluated = {"dynamicProps":true,"dynamicItems":false};


function validate261(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate261.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((data.anchor_state === undefined) && (missing0 = "anchor_state")) || ((data.request === undefined) && (missing0 = "request"))) || ((data.interpretation === undefined) && (missing0 = "interpretation"))) || ((data.objective === undefined) && (missing0 = "objective"))) || ((data.decisions === undefined) && (missing0 = "decisions"))){
validate261.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(((((((key0 === "anchor_state") || (key0 === "decisions")) || (key0 === "interpretation")) || (key0 === "objective")) || (key0 === "parent")) || (key0 === "request")) || (key0 === "success_value"))){
validate261.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.anchor_state !== undefined){
const _errs2 = errors;
if(typeof data.anchor_state !== "string"){
validate261.errors = [{instancePath:instancePath+"/anchor_state",schemaPath:"#/properties/anchor_state/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.decisions !== undefined){
const _errs4 = errors;
if(!(validate262(data.decisions, {instancePath:instancePath+"/decisions",parentData:data,parentDataProperty:"decisions",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate262.errors : vErrors.concat(validate262.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.interpretation !== undefined){
const _errs5 = errors;
if(typeof data.interpretation !== "string"){
validate261.errors = [{instancePath:instancePath+"/interpretation",schemaPath:"#/properties/interpretation/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
const _errs7 = errors;
if(!(validate272(data.objective, {instancePath:instancePath+"/objective",parentData:data,parentDataProperty:"objective",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate272.errors : vErrors.concat(validate272.errors);
errors = vErrors.length;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.parent !== undefined){
let data4 = data.parent;
const _errs8 = errors;
if((typeof data4 !== "string") && (data4 !== null)){
validate261.errors = [{instancePath:instancePath+"/parent",schemaPath:"#/properties/parent/type",keyword:"type",params:{type: schema106.properties.parent.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.request !== undefined){
const _errs10 = errors;
if(typeof data.request !== "string"){
validate261.errors = [{instancePath:instancePath+"/request",schemaPath:"#/properties/request/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.success_value !== undefined){
let data6 = data.success_value;
const _errs12 = errors;
if((!((typeof data6 == "number") && (isFinite(data6)))) && (data6 !== null)){
validate261.errors = [{instancePath:instancePath+"/success_value",schemaPath:"#/properties/success_value/type",keyword:"type",params:{type: schema106.properties.success_value.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
else {
validate261.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate261.errors = vErrors;
return errors === 0;
}
validate261.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

const schema114 = {"properties":{"approximations":{"items":{"$ref":"#/$defs/CostApproximation"},"type":"array"},"method":{"$ref":"#/$defs/CostPreparation"},"model":{"type":"string"},"quantity":{"type":"string"},"source":{"type":"string"},"units":{"type":"string"}},"required":["method","source","model","quantity","units","approximations"],"type":"object"};
const schema115 = {"properties":{"bus":{"format":"uint","minimum":0,"type":"integer"},"generator":{"format":"uint","minimum":0,"type":"integer"},"max_breakpoint_error":{"format":"double","type":"number"},"model_cost":{"$ref":"#/$defs/GenCost"},"rms_breakpoint_error":{"format":"double","type":"number"},"source_cost":{"$ref":"#/$defs/GenCost"}},"required":["generator","bus","source_cost","model_cost","rms_breakpoint_error","max_breakpoint_error"],"type":"object"};
const schema116 = {"description":"A generator cost curve (`mpc.gencost` row).","properties":{"coeffs":{"description":"Raw coefficients, highest order first for the polynomial model:\n`[c_{k-1}, …, c1, c0]`.","items":{"format":"double","type":"number"},"type":"array"},"model":{"description":"1 = piecewise linear, 2 = polynomial.","format":"uint8","maximum":255,"minimum":0,"type":"integer"},"ncost":{"description":"Number of cost coefficients (polynomial) or breakpoints (piecewise).","format":"uint","minimum":0,"type":"integer"},"shutdown":{"format":"double","type":"number"},"startup":{"format":"double","type":"number"}},"required":["model","startup","shutdown","ncost","coeffs"],"type":"object"};

function validate281(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate281.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((data.model === undefined) && (missing0 = "model")) || ((data.startup === undefined) && (missing0 = "startup"))) || ((data.shutdown === undefined) && (missing0 = "shutdown"))) || ((data.ncost === undefined) && (missing0 = "ncost"))) || ((data.coeffs === undefined) && (missing0 = "coeffs"))){
validate281.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.coeffs !== undefined){
let data0 = data.coeffs;
const _errs1 = errors;
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
let data1 = data0[i0];
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate281.errors = [{instancePath:instancePath+"/coeffs/" + i0,schemaPath:"#/properties/coeffs/items/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
else {
validate281.errors = [{instancePath:instancePath+"/coeffs",schemaPath:"#/properties/coeffs/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.model !== undefined){
let data2 = data.model;
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate281.errors = [{instancePath:instancePath+"/model",schemaPath:"#/properties/model/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 > 255 || isNaN(data2)){
validate281.errors = [{instancePath:instancePath+"/model",schemaPath:"#/properties/model/maximum",keyword:"maximum",params:{comparison: "<=", limit: 255},message:"must be <= 255"}];
return false;
}
else {
if(data2 < 0 || isNaN(data2)){
validate281.errors = [{instancePath:instancePath+"/model",schemaPath:"#/properties/model/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.ncost !== undefined){
let data3 = data.ncost;
const _errs7 = errors;
if(!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))){
validate281.errors = [{instancePath:instancePath+"/ncost",schemaPath:"#/properties/ncost/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs7){
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 0 || isNaN(data3)){
validate281.errors = [{instancePath:instancePath+"/ncost",schemaPath:"#/properties/ncost/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.shutdown !== undefined){
let data4 = data.shutdown;
const _errs9 = errors;
if(errors === _errs9){
if(!((typeof data4 == "number") && (isFinite(data4)))){
validate281.errors = [{instancePath:instancePath+"/shutdown",schemaPath:"#/properties/shutdown/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.startup !== undefined){
let data5 = data.startup;
const _errs11 = errors;
if(errors === _errs11){
if(!((typeof data5 == "number") && (isFinite(data5)))){
validate281.errors = [{instancePath:instancePath+"/startup",schemaPath:"#/properties/startup/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
else {
validate281.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate281.errors = vErrors;
return errors === 0;
}
validate281.evaluated = {"props":{"coeffs":true,"model":true,"ncost":true,"shutdown":true,"startup":true},"dynamicProps":false,"dynamicItems":false};


function validate280(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate280.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((data.generator === undefined) && (missing0 = "generator")) || ((data.bus === undefined) && (missing0 = "bus"))) || ((data.source_cost === undefined) && (missing0 = "source_cost"))) || ((data.model_cost === undefined) && (missing0 = "model_cost"))) || ((data.rms_breakpoint_error === undefined) && (missing0 = "rms_breakpoint_error"))) || ((data.max_breakpoint_error === undefined) && (missing0 = "max_breakpoint_error"))){
validate280.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate280.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate280.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.generator !== undefined){
let data1 = data.generator;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate280.errors = [{instancePath:instancePath+"/generator",schemaPath:"#/properties/generator/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate280.errors = [{instancePath:instancePath+"/generator",schemaPath:"#/properties/generator/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.max_breakpoint_error !== undefined){
let data2 = data.max_breakpoint_error;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate280.errors = [{instancePath:instancePath+"/max_breakpoint_error",schemaPath:"#/properties/max_breakpoint_error/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.model_cost !== undefined){
const _errs7 = errors;
if(!(validate281(data.model_cost, {instancePath:instancePath+"/model_cost",parentData:data,parentDataProperty:"model_cost",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate281.errors : vErrors.concat(validate281.errors);
errors = vErrors.length;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rms_breakpoint_error !== undefined){
let data4 = data.rms_breakpoint_error;
const _errs8 = errors;
if(errors === _errs8){
if(!((typeof data4 == "number") && (isFinite(data4)))){
validate280.errors = [{instancePath:instancePath+"/rms_breakpoint_error",schemaPath:"#/properties/rms_breakpoint_error/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.source_cost !== undefined){
const _errs10 = errors;
if(!(validate281(data.source_cost, {instancePath:instancePath+"/source_cost",parentData:data,parentDataProperty:"source_cost",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate281.errors : vErrors.concat(validate281.errors);
errors = vErrors.length;
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
else {
validate280.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate280.errors = vErrors;
return errors === 0;
}
validate280.evaluated = {"props":{"bus":true,"generator":true,"max_breakpoint_error":true,"model_cost":true,"rms_breakpoint_error":true,"source_cost":true},"dynamicProps":false,"dynamicItems":false};

const schema117 = {"enum":["exact","convex_quadratic_fit"],"type":"string"};

function validate285(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate285.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate285.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "exact") || (data === "convex_quadratic_fit"))){
validate285.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema117.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate285.errors = vErrors;
return errors === 0;
}
validate285.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate279(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate279.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((data.method === undefined) && (missing0 = "method")) || ((data.source === undefined) && (missing0 = "source"))) || ((data.model === undefined) && (missing0 = "model"))) || ((data.quantity === undefined) && (missing0 = "quantity"))) || ((data.units === undefined) && (missing0 = "units"))) || ((data.approximations === undefined) && (missing0 = "approximations"))){
validate279.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.approximations !== undefined){
let data0 = data.approximations;
const _errs1 = errors;
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate280(data0[i0], {instancePath:instancePath+"/approximations/" + i0,parentData:data0,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate280.errors : vErrors.concat(validate280.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
else {
validate279.errors = [{instancePath:instancePath+"/approximations",schemaPath:"#/properties/approximations/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.method !== undefined){
const _errs4 = errors;
if(!(validate285(data.method, {instancePath:instancePath+"/method",parentData:data,parentDataProperty:"method",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate285.errors : vErrors.concat(validate285.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.model !== undefined){
const _errs5 = errors;
if(typeof data.model !== "string"){
validate279.errors = [{instancePath:instancePath+"/model",schemaPath:"#/properties/model/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.quantity !== undefined){
const _errs7 = errors;
if(typeof data.quantity !== "string"){
validate279.errors = [{instancePath:instancePath+"/quantity",schemaPath:"#/properties/quantity/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.source !== undefined){
const _errs9 = errors;
if(typeof data.source !== "string"){
validate279.errors = [{instancePath:instancePath+"/source",schemaPath:"#/properties/source/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.units !== undefined){
const _errs11 = errors;
if(typeof data.units !== "string"){
validate279.errors = [{instancePath:instancePath+"/units",schemaPath:"#/properties/units/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
else {
validate279.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate279.errors = vErrors;
return errors === 0;
}
validate279.evaluated = {"props":{"approximations":true,"method":true,"model":true,"quantity":true,"source":true,"units":true},"dynamicProps":false,"dynamicItems":false};

const schema118 = {"additionalProperties":false,"description":"An immutable electrical state. Inspection never implies application.","properties":{"formulation":{"$ref":"#/$defs/Problem"},"input":{"type":"string"},"label":{"type":"string"},"parent":{"type":["string","null"]},"solution":{"type":["string","null"]},"view":{"type":["string","null"]}},"required":["formulation","input","label"],"type":"object"};

function validate288(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate288.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.formulation === undefined) && (missing0 = "formulation")) || ((data.input === undefined) && (missing0 = "input"))) || ((data.label === undefined) && (missing0 = "label"))){
validate288.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!((((((key0 === "formulation") || (key0 === "input")) || (key0 === "label")) || (key0 === "parent")) || (key0 === "solution")) || (key0 === "view"))){
validate288.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.formulation !== undefined){
const _errs2 = errors;
if(!(validate120(data.formulation, {instancePath:instancePath+"/formulation",parentData:data,parentDataProperty:"formulation",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate120.errors : vErrors.concat(validate120.errors);
errors = vErrors.length;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.input !== undefined){
const _errs3 = errors;
if(typeof data.input !== "string"){
validate288.errors = [{instancePath:instancePath+"/input",schemaPath:"#/properties/input/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.label !== undefined){
const _errs5 = errors;
if(typeof data.label !== "string"){
validate288.errors = [{instancePath:instancePath+"/label",schemaPath:"#/properties/label/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.parent !== undefined){
let data3 = data.parent;
const _errs7 = errors;
if((typeof data3 !== "string") && (data3 !== null)){
validate288.errors = [{instancePath:instancePath+"/parent",schemaPath:"#/properties/parent/type",keyword:"type",params:{type: schema118.properties.parent.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.solution !== undefined){
let data4 = data.solution;
const _errs9 = errors;
if((typeof data4 !== "string") && (data4 !== null)){
validate288.errors = [{instancePath:instancePath+"/solution",schemaPath:"#/properties/solution/type",keyword:"type",params:{type: schema118.properties.solution.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.view !== undefined){
let data5 = data.view;
const _errs11 = errors;
if((typeof data5 !== "string") && (data5 !== null)){
validate288.errors = [{instancePath:instancePath+"/view",schemaPath:"#/properties/view/type",keyword:"type",params:{type: schema118.properties.view.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate288.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate288.errors = vErrors;
return errors === 0;
}
validate288.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};


function validate244(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate244.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((((((data.schema === undefined) && (missing0 = "schema")) || ((data.version === undefined) && (missing0 = "version"))) || ((data.id === undefined) && (missing0 = "id"))) || ((data.title === undefined) && (missing0 = "title"))) || ((data.revision === undefined) && (missing0 = "revision"))) || ((data.goals === undefined) && (missing0 = "goals"))) || ((data.states === undefined) && (missing0 = "states"))) || ((data.experiments === undefined) && (missing0 = "experiments"))) || ((data.experiment_order === undefined) && (missing0 = "experiment_order"))) || ((data.decisions === undefined) && (missing0 = "decisions"))){
validate244.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(func1.call(schema97.properties, key0))){
validate244.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.active_goal !== undefined){
let data0 = data.active_goal;
const _errs2 = errors;
if((typeof data0 !== "string") && (data0 !== null)){
validate244.errors = [{instancePath:instancePath+"/active_goal",schemaPath:"#/properties/active_goal/type",keyword:"type",params:{type: schema97.properties.active_goal.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.applied_state !== undefined){
let data1 = data.applied_state;
const _errs4 = errors;
if((typeof data1 !== "string") && (data1 !== null)){
validate244.errors = [{instancePath:instancePath+"/applied_state",schemaPath:"#/properties/applied_state/type",keyword:"type",params:{type: schema97.properties.applied_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.base_input !== undefined){
let data2 = data.base_input;
const _errs6 = errors;
if((typeof data2 !== "string") && (data2 !== null)){
validate244.errors = [{instancePath:instancePath+"/base_input",schemaPath:"#/properties/base_input/type",keyword:"type",params:{type: schema97.properties.base_input.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.decisions !== undefined){
let data3 = data.decisions;
const _errs8 = errors;
if(errors === _errs8){
if(data3 && typeof data3 == "object" && !Array.isArray(data3)){
for(const key1 in data3){
const _errs11 = errors;
if(!(validate245(data3[key1], {instancePath:instancePath+"/decisions/" + key1.replace(/~/g, "~0").replace(/\//g, "~1"),parentData:data3,parentDataProperty:key1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate245.errors : vErrors.concat(validate245.errors);
errors = vErrors.length;
}
var valid1 = _errs11 === errors;
if(!valid1){
break;
}
}
}
else {
validate244.errors = [{instancePath:instancePath+"/decisions",schemaPath:"#/properties/decisions/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.display !== undefined){
let data5 = data.display;
const _errs12 = errors;
const _errs13 = errors;
let valid2 = false;
const _errs14 = errors;
if(!(validate249(data5, {instancePath:instancePath+"/display",parentData:data,parentDataProperty:"display",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate249.errors : vErrors.concat(validate249.errors);
errors = vErrors.length;
}
var _valid0 = _errs14 === errors;
valid2 = valid2 || _valid0;
const _errs15 = errors;
if(data5 !== null){
const err0 = {instancePath:instancePath+"/display",schemaPath:"#/properties/display/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs15 === errors;
valid2 = valid2 || _valid0;
if(!valid2){
const err1 = {instancePath:instancePath+"/display",schemaPath:"#/properties/display/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate244.errors = vErrors;
return false;
}
else {
errors = _errs13;
if(vErrors !== null){
if(_errs13){
vErrors.length = _errs13;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.experiment_order !== undefined){
let data6 = data.experiment_order;
const _errs17 = errors;
if(errors === _errs17){
if(Array.isArray(data6)){
var valid3 = true;
const len0 = data6.length;
for(let i0=0; i0<len0; i0++){
const _errs19 = errors;
if(typeof data6[i0] !== "string"){
validate244.errors = [{instancePath:instancePath+"/experiment_order/" + i0,schemaPath:"#/properties/experiment_order/items/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid3 = _errs19 === errors;
if(!valid3){
break;
}
}
}
else {
validate244.errors = [{instancePath:instancePath+"/experiment_order",schemaPath:"#/properties/experiment_order/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs17 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.experiments !== undefined){
let data8 = data.experiments;
const _errs21 = errors;
if(errors === _errs21){
if(data8 && typeof data8 == "object" && !Array.isArray(data8)){
for(const key2 in data8){
const _errs24 = errors;
if(!(validate255(data8[key2], {instancePath:instancePath+"/experiments/" + key2.replace(/~/g, "~0").replace(/\//g, "~1"),parentData:data8,parentDataProperty:key2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate255.errors : vErrors.concat(validate255.errors);
errors = vErrors.length;
}
var valid4 = _errs24 === errors;
if(!valid4){
break;
}
}
}
else {
validate244.errors = [{instancePath:instancePath+"/experiments",schemaPath:"#/properties/experiments/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.goals !== undefined){
let data10 = data.goals;
const _errs25 = errors;
if(errors === _errs25){
if(data10 && typeof data10 == "object" && !Array.isArray(data10)){
for(const key3 in data10){
const _errs28 = errors;
if(!(validate261(data10[key3], {instancePath:instancePath+"/goals/" + key3.replace(/~/g, "~0").replace(/\//g, "~1"),parentData:data10,parentDataProperty:key3,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate261.errors : vErrors.concat(validate261.errors);
errors = vErrors.length;
}
var valid5 = _errs28 === errors;
if(!valid5){
break;
}
}
}
else {
validate244.errors = [{instancePath:instancePath+"/goals",schemaPath:"#/properties/goals/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs25 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.id !== undefined){
const _errs29 = errors;
if(typeof data.id !== "string"){
validate244.errors = [{instancePath:instancePath+"/id",schemaPath:"#/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs29 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.inspected_state !== undefined){
let data13 = data.inspected_state;
const _errs31 = errors;
if((typeof data13 !== "string") && (data13 !== null)){
validate244.errors = [{instancePath:instancePath+"/inspected_state",schemaPath:"#/properties/inspected_state/type",keyword:"type",params:{type: schema97.properties.inspected_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs31 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.model_details !== undefined){
let data14 = data.model_details;
const _errs33 = errors;
const _errs34 = errors;
let valid6 = false;
const _errs35 = errors;
if(!(validate279(data14, {instancePath:instancePath+"/model_details",parentData:data,parentDataProperty:"model_details",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate279.errors : vErrors.concat(validate279.errors);
errors = vErrors.length;
}
var _valid1 = _errs35 === errors;
valid6 = valid6 || _valid1;
if(_valid1){
var props1 = {};
props1.approximations = true;
props1.method = true;
props1.model = true;
props1.quantity = true;
props1.source = true;
props1.units = true;
}
const _errs36 = errors;
if(data14 !== null){
const err2 = {instancePath:instancePath+"/model_details",schemaPath:"#/properties/model_details/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs36 === errors;
valid6 = valid6 || _valid1;
if(!valid6){
const err3 = {instancePath:instancePath+"/model_details",schemaPath:"#/properties/model_details/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate244.errors = vErrors;
return false;
}
else {
errors = _errs34;
if(vErrors !== null){
if(_errs34){
vErrors.length = _errs34;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs33 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.recommended_state !== undefined){
let data15 = data.recommended_state;
const _errs38 = errors;
if((typeof data15 !== "string") && (data15 !== null)){
validate244.errors = [{instancePath:instancePath+"/recommended_state",schemaPath:"#/properties/recommended_state/type",keyword:"type",params:{type: schema97.properties.recommended_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs38 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.revision !== undefined){
let data16 = data.revision;
const _errs40 = errors;
if(!(((typeof data16 == "number") && (!(data16 % 1) && !isNaN(data16))) && (isFinite(data16)))){
validate244.errors = [{instancePath:instancePath+"/revision",schemaPath:"#/properties/revision/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs40){
if((typeof data16 == "number") && (isFinite(data16))){
if(data16 < 0 || isNaN(data16)){
validate244.errors = [{instancePath:instancePath+"/revision",schemaPath:"#/properties/revision/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs40 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.schema !== undefined){
const _errs42 = errors;
if(typeof data.schema !== "string"){
validate244.errors = [{instancePath:instancePath+"/schema",schemaPath:"#/properties/schema/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs42 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.states !== undefined){
let data18 = data.states;
const _errs44 = errors;
if(errors === _errs44){
if(data18 && typeof data18 == "object" && !Array.isArray(data18)){
for(const key4 in data18){
const _errs47 = errors;
if(!(validate288(data18[key4], {instancePath:instancePath+"/states/" + key4.replace(/~/g, "~0").replace(/\//g, "~1"),parentData:data18,parentDataProperty:key4,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate288.errors : vErrors.concat(validate288.errors);
errors = vErrors.length;
}
var valid7 = _errs47 === errors;
if(!valid7){
break;
}
}
}
else {
validate244.errors = [{instancePath:instancePath+"/states",schemaPath:"#/properties/states/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs44 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.title !== undefined){
const _errs48 = errors;
if(typeof data.title !== "string"){
validate244.errors = [{instancePath:instancePath+"/title",schemaPath:"#/properties/title/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs48 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.version !== undefined){
let data21 = data.version;
const _errs50 = errors;
if(!(((typeof data21 == "number") && (!(data21 % 1) && !isNaN(data21))) && (isFinite(data21)))){
validate244.errors = [{instancePath:instancePath+"/version",schemaPath:"#/properties/version/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs50){
if((typeof data21 == "number") && (isFinite(data21))){
if(data21 < 0 || isNaN(data21)){
validate244.errors = [{instancePath:instancePath+"/version",schemaPath:"#/properties/version/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs50 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate244.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate244.errors = vErrors;
return errors === 0;
}
validate244.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};


function validate398(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate398.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.document === undefined) && (missing0 = "document")) || ((data.artifacts === undefined) && (missing0 = "artifacts"))){
validate398.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!((key0 === "artifacts") || (key0 === "document"))){
validate398.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.artifacts !== undefined){
let data0 = data.artifacts;
const _errs2 = errors;
if(errors === _errs2){
if(data0 && typeof data0 == "object" && !Array.isArray(data0)){
for(const key1 in data0){
const _errs5 = errors;
if(!(validate240(data0[key1], {instancePath:instancePath+"/artifacts/" + key1.replace(/~/g, "~0").replace(/\//g, "~1"),parentData:data0,parentDataProperty:key1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate240.errors : vErrors.concat(validate240.errors);
errors = vErrors.length;
}
var valid1 = _errs5 === errors;
if(!valid1){
break;
}
}
}
else {
validate398.errors = [{instancePath:instancePath+"/artifacts",schemaPath:"#/properties/artifacts/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.document !== undefined){
const _errs6 = errors;
if(!(validate244(data.document, {instancePath:instancePath+"/document",parentData:data,parentDataProperty:"document",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate244.errors : vErrors.concat(validate244.errors);
errors = vErrors.length;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate398.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate398.errors = vErrors;
return errors === 0;
}
validate398.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

export const study_result = validate401;
const schema119 = {"additionalProperties":false,"properties":{"comparison":{"anyOf":[{"$ref":"#/$defs/Comparison"},{"type":"null"}]},"demand_changes":{"default":null,"items":{"$ref":"#/$defs/DemandChange"},"type":["array","null"]},"experiment":{"type":["string","null"]},"inspected_view":{"anyOf":[{"$ref":"#/$defs/SolveResponse"},{"type":"null"}]},"summary":{"$ref":"#/$defs/StudySummary"}},"required":["summary"],"type":"object"};
const schema120 = {"additionalProperties":false,"properties":{"goal":{"type":["string","null"]},"improvement":{"format":"double","type":["number","null"]},"left":{"type":"string"},"left_value":{"format":"double","type":["number","null"]},"left_view":{"anyOf":[{"$ref":"#/$defs/SolveResponse"},{"type":"null"}]},"right":{"type":"string"},"right_value":{"format":"double","type":["number","null"]},"right_view":{"anyOf":[{"$ref":"#/$defs/SolveResponse"},{"type":"null"}]}},"required":["left","right"],"type":"object"};
const schema121 = {"description":"The formulation-agnostic solve result. A superset: every block is optional, and\neach formulation fills what it produces. Powers are MW/MVAr, nodal values are\nin objective units per selected power unit, angles radians, `vm` per unit, and\n`w = |V|^2` per unit squared. Element ids\nare the original bus/branch/generator ids, so the frontend joins straight onto\nits case.","properties":{"branch_constraints":{"description":"Thermal constraint state and marginal relief, when exposed by the formulation.","items":{"$ref":"#/$defs/BranchConstraintResult"},"type":["array","null"]},"branch_susceptance_formula":{"anyOf":[{"$ref":"#/$defs/DcFormula"},{"type":"null"}],"description":"Resolved DC convention; absent for AC results and older saved responses."},"constraints":{"description":"The request's linear constraints, in request order (dcopf). Absent when\nthe request stated none.","items":{"$ref":"#/$defs/ConstraintResult"},"type":["array","null"]},"diagnostics":{"description":"How the solve treated the network's islands. Empty, and omitted, when\nevery island is supplied and referenced as stated.","items":{"$ref":"#/$defs/SolveDiagnostic"},"type":"array"},"dispatch":{"items":{"$ref":"#/$defs/GenDispatch"},"type":["array","null"]},"flows":{"items":{"$ref":"#/$defs/BranchFlow"},"type":["array","null"]},"formulation":{"$ref":"#/$defs/Problem","description":"The formulation that produced this, echoed for the client."},"initialization":{"anyOf":[{"$ref":"#/$defs/AcPfInitialization"},{"type":"null"}],"description":"Successful AC power-flow start. Absent for other formulations and legacy results."},"injections":{"description":"Nodal injections (acpf), MW/MVAr.","items":{"$ref":"#/$defs/BusInjection"},"type":["array","null"]},"iterations":{"anyOf":[{"$ref":"#/$defs/Iterations"},{"type":"null"}]},"limit_rounds":{"description":"The solves a DC OPF ran under the request's `limits` (dcopf with\n`limits` only).","items":{"$ref":"#/$defs/LimitRound"},"type":["array","null"]},"lmp":{"description":"Active nodal price when the solved OPF declares a network generator\ncost objective. Feasibility solves leave this absent.","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"lmp_q":{"description":"Reactive nodal price. Always `None` in this build; the field is retained for\nwire-format stability.","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"objective":{"format":"double","type":["number","null"]},"sensitivities":{"description":"One self-describing matrix per requested cell, in request order. Each carries\nits own row/column element ids and the served-unit label.","items":{"$ref":"#/$defs/SensitivityMatrix"},"type":"array"},"status":{"$ref":"#/$defs/SolveStatus"},"va":{"description":"Voltage angle, radians (every path except socwr, which is W-space).","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"vm":{"description":"Voltage magnitude, per unit (ACPF or square root of SOCWR squared magnitude).","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"w":{"description":"Squared voltage magnitude `w = |V|^2`, per unit squared (socwr). The conic\nrelaxation does not guarantee a globally consistent set of voltage angles.","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"wi":{"items":{"$ref":"#/$defs/BranchScalar"},"type":["array","null"]},"wr":{"description":"Real and imaginary oriented branch voltage products (SOCWR).","items":{"$ref":"#/$defs/BranchScalar"},"type":["array","null"]}},"required":["formulation","status"],"type":"object"};

function validate295(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate295.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.formulation === undefined) && (missing0 = "formulation")) || ((data.status === undefined) && (missing0 = "status"))){
validate295.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch_constraints !== undefined){
let data0 = data.branch_constraints;
const _errs1 = errors;
if((!(Array.isArray(data0))) && (data0 !== null)){
validate295.errors = [{instancePath:instancePath+"/branch_constraints",schemaPath:"#/properties/branch_constraints/type",keyword:"type",params:{type: schema121.properties.branch_constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate100(data0[i0], {instancePath:instancePath+"/branch_constraints/" + i0,parentData:data0,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate100.errors : vErrors.concat(validate100.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.branch_susceptance_formula !== undefined){
let data2 = data.branch_susceptance_formula;
const _errs4 = errors;
const _errs5 = errors;
let valid2 = false;
const _errs6 = errors;
if(!(validate102(data2, {instancePath:instancePath+"/branch_susceptance_formula",parentData:data,parentDataProperty:"branch_susceptance_formula",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate102.errors : vErrors.concat(validate102.errors);
errors = vErrors.length;
}
var _valid0 = _errs6 === errors;
valid2 = valid2 || _valid0;
const _errs7 = errors;
if(data2 !== null){
const err0 = {instancePath:instancePath+"/branch_susceptance_formula",schemaPath:"#/properties/branch_susceptance_formula/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs7 === errors;
valid2 = valid2 || _valid0;
if(!valid2){
const err1 = {instancePath:instancePath+"/branch_susceptance_formula",schemaPath:"#/properties/branch_susceptance_formula/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate295.errors = vErrors;
return false;
}
else {
errors = _errs5;
if(vErrors !== null){
if(_errs5){
vErrors.length = _errs5;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.constraints !== undefined){
let data3 = data.constraints;
const _errs9 = errors;
if((!(Array.isArray(data3))) && (data3 !== null)){
validate295.errors = [{instancePath:instancePath+"/constraints",schemaPath:"#/properties/constraints/type",keyword:"type",params:{type: schema121.properties.constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs9){
if(Array.isArray(data3)){
var valid3 = true;
const len1 = data3.length;
for(let i1=0; i1<len1; i1++){
const _errs11 = errors;
if(!(validate104(data3[i1], {instancePath:instancePath+"/constraints/" + i1,parentData:data3,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate104.errors : vErrors.concat(validate104.errors);
errors = vErrors.length;
}
var valid3 = _errs11 === errors;
if(!valid3){
break;
}
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.diagnostics !== undefined){
let data5 = data.diagnostics;
const _errs12 = errors;
if(errors === _errs12){
if(Array.isArray(data5)){
var valid4 = true;
const len2 = data5.length;
for(let i2=0; i2<len2; i2++){
const _errs14 = errors;
if(!(validate106(data5[i2], {instancePath:instancePath+"/diagnostics/" + i2,parentData:data5,parentDataProperty:i2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate106.errors : vErrors.concat(validate106.errors);
errors = vErrors.length;
}
var valid4 = _errs14 === errors;
if(!valid4){
break;
}
}
}
else {
validate295.errors = [{instancePath:instancePath+"/diagnostics",schemaPath:"#/properties/diagnostics/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.dispatch !== undefined){
let data7 = data.dispatch;
const _errs15 = errors;
if((!(Array.isArray(data7))) && (data7 !== null)){
validate295.errors = [{instancePath:instancePath+"/dispatch",schemaPath:"#/properties/dispatch/type",keyword:"type",params:{type: schema121.properties.dispatch.type},message:"must be array,null"}];
return false;
}
if(errors === _errs15){
if(Array.isArray(data7)){
var valid5 = true;
const len3 = data7.length;
for(let i3=0; i3<len3; i3++){
const _errs17 = errors;
if(!(validate108(data7[i3], {instancePath:instancePath+"/dispatch/" + i3,parentData:data7,parentDataProperty:i3,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate108.errors : vErrors.concat(validate108.errors);
errors = vErrors.length;
}
var valid5 = _errs17 === errors;
if(!valid5){
break;
}
}
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.flows !== undefined){
let data9 = data.flows;
const _errs18 = errors;
if((!(Array.isArray(data9))) && (data9 !== null)){
validate295.errors = [{instancePath:instancePath+"/flows",schemaPath:"#/properties/flows/type",keyword:"type",params:{type: schema121.properties.flows.type},message:"must be array,null"}];
return false;
}
if(errors === _errs18){
if(Array.isArray(data9)){
var valid6 = true;
const len4 = data9.length;
for(let i4=0; i4<len4; i4++){
const _errs20 = errors;
if(!(validate110(data9[i4], {instancePath:instancePath+"/flows/" + i4,parentData:data9,parentDataProperty:i4,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate110.errors : vErrors.concat(validate110.errors);
errors = vErrors.length;
}
var valid6 = _errs20 === errors;
if(!valid6){
break;
}
}
}
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.formulation !== undefined){
const _errs21 = errors;
if(!(validate120(data.formulation, {instancePath:instancePath+"/formulation",parentData:data,parentDataProperty:"formulation",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate120.errors : vErrors.concat(validate120.errors);
errors = vErrors.length;
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.initialization !== undefined){
let data12 = data.initialization;
const _errs22 = errors;
const _errs23 = errors;
let valid7 = false;
const _errs24 = errors;
if(!(validate122(data12, {instancePath:instancePath+"/initialization",parentData:data,parentDataProperty:"initialization",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate122.errors : vErrors.concat(validate122.errors);
errors = vErrors.length;
}
var _valid1 = _errs24 === errors;
valid7 = valid7 || _valid1;
if(_valid1){
var props0 = {};
props0.attempts = true;
props0.requested = true;
props0.restart = true;
props0.used = true;
}
const _errs25 = errors;
if(data12 !== null){
const err2 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs25 === errors;
valid7 = valid7 || _valid1;
if(!valid7){
const err3 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate295.errors = vErrors;
return false;
}
else {
errors = _errs23;
if(vErrors !== null){
if(_errs23){
vErrors.length = _errs23;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs22 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.injections !== undefined){
let data13 = data.injections;
const _errs27 = errors;
if((!(Array.isArray(data13))) && (data13 !== null)){
validate295.errors = [{instancePath:instancePath+"/injections",schemaPath:"#/properties/injections/type",keyword:"type",params:{type: schema121.properties.injections.type},message:"must be array,null"}];
return false;
}
if(errors === _errs27){
if(Array.isArray(data13)){
var valid8 = true;
const len5 = data13.length;
for(let i5=0; i5<len5; i5++){
const _errs29 = errors;
if(!(validate128(data13[i5], {instancePath:instancePath+"/injections/" + i5,parentData:data13,parentDataProperty:i5,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate128.errors : vErrors.concat(validate128.errors);
errors = vErrors.length;
}
var valid8 = _errs29 === errors;
if(!valid8){
break;
}
}
}
}
var valid0 = _errs27 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iterations !== undefined){
let data15 = data.iterations;
const _errs30 = errors;
const _errs31 = errors;
let valid9 = false;
const _errs32 = errors;
if(!(validate60(data15, {instancePath:instancePath+"/iterations",parentData:data,parentDataProperty:"iterations",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate60.errors : vErrors.concat(validate60.errors);
errors = vErrors.length;
}
var _valid2 = _errs32 === errors;
valid9 = valid9 || _valid2;
const _errs33 = errors;
if(data15 !== null){
const err4 = {instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
var _valid2 = _errs33 === errors;
valid9 = valid9 || _valid2;
if(!valid9){
const err5 = {instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
validate295.errors = vErrors;
return false;
}
else {
errors = _errs31;
if(vErrors !== null){
if(_errs31){
vErrors.length = _errs31;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs30 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.limit_rounds !== undefined){
let data16 = data.limit_rounds;
const _errs35 = errors;
if((!(Array.isArray(data16))) && (data16 !== null)){
validate295.errors = [{instancePath:instancePath+"/limit_rounds",schemaPath:"#/properties/limit_rounds/type",keyword:"type",params:{type: schema121.properties.limit_rounds.type},message:"must be array,null"}];
return false;
}
if(errors === _errs35){
if(Array.isArray(data16)){
var valid10 = true;
const len6 = data16.length;
for(let i6=0; i6<len6; i6++){
const _errs37 = errors;
if(!(validate131(data16[i6], {instancePath:instancePath+"/limit_rounds/" + i6,parentData:data16,parentDataProperty:i6,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate131.errors : vErrors.concat(validate131.errors);
errors = vErrors.length;
}
var valid10 = _errs37 === errors;
if(!valid10){
break;
}
}
}
}
var valid0 = _errs35 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lmp !== undefined){
let data18 = data.lmp;
const _errs38 = errors;
if((!(Array.isArray(data18))) && (data18 !== null)){
validate295.errors = [{instancePath:instancePath+"/lmp",schemaPath:"#/properties/lmp/type",keyword:"type",params:{type: schema121.properties.lmp.type},message:"must be array,null"}];
return false;
}
if(errors === _errs38){
if(Array.isArray(data18)){
var valid11 = true;
const len7 = data18.length;
for(let i7=0; i7<len7; i7++){
const _errs40 = errors;
if(!(validate133(data18[i7], {instancePath:instancePath+"/lmp/" + i7,parentData:data18,parentDataProperty:i7,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid11 = _errs40 === errors;
if(!valid11){
break;
}
}
}
}
var valid0 = _errs38 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lmp_q !== undefined){
let data20 = data.lmp_q;
const _errs41 = errors;
if((!(Array.isArray(data20))) && (data20 !== null)){
validate295.errors = [{instancePath:instancePath+"/lmp_q",schemaPath:"#/properties/lmp_q/type",keyword:"type",params:{type: schema121.properties.lmp_q.type},message:"must be array,null"}];
return false;
}
if(errors === _errs41){
if(Array.isArray(data20)){
var valid12 = true;
const len8 = data20.length;
for(let i8=0; i8<len8; i8++){
const _errs43 = errors;
if(!(validate133(data20[i8], {instancePath:instancePath+"/lmp_q/" + i8,parentData:data20,parentDataProperty:i8,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid12 = _errs43 === errors;
if(!valid12){
break;
}
}
}
}
var valid0 = _errs41 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
let data22 = data.objective;
const _errs44 = errors;
if((!((typeof data22 == "number") && (isFinite(data22)))) && (data22 !== null)){
validate295.errors = [{instancePath:instancePath+"/objective",schemaPath:"#/properties/objective/type",keyword:"type",params:{type: schema121.properties.objective.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs44 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.sensitivities !== undefined){
let data23 = data.sensitivities;
const _errs46 = errors;
if(errors === _errs46){
if(Array.isArray(data23)){
var valid13 = true;
const len9 = data23.length;
for(let i9=0; i9<len9; i9++){
const _errs48 = errors;
if(!(validate66(data23[i9], {instancePath:instancePath+"/sensitivities/" + i9,parentData:data23,parentDataProperty:i9,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate66.errors : vErrors.concat(validate66.errors);
errors = vErrors.length;
}
var valid13 = _errs48 === errors;
if(!valid13){
break;
}
}
}
else {
validate295.errors = [{instancePath:instancePath+"/sensitivities",schemaPath:"#/properties/sensitivities/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs46 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.status !== undefined){
const _errs49 = errors;
if(!(validate139(data.status, {instancePath:instancePath+"/status",parentData:data,parentDataProperty:"status",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate139.errors : vErrors.concat(validate139.errors);
errors = vErrors.length;
}
var valid0 = _errs49 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.va !== undefined){
let data26 = data.va;
const _errs50 = errors;
if((!(Array.isArray(data26))) && (data26 !== null)){
validate295.errors = [{instancePath:instancePath+"/va",schemaPath:"#/properties/va/type",keyword:"type",params:{type: schema121.properties.va.type},message:"must be array,null"}];
return false;
}
if(errors === _errs50){
if(Array.isArray(data26)){
var valid14 = true;
const len10 = data26.length;
for(let i10=0; i10<len10; i10++){
const _errs52 = errors;
if(!(validate133(data26[i10], {instancePath:instancePath+"/va/" + i10,parentData:data26,parentDataProperty:i10,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid14 = _errs52 === errors;
if(!valid14){
break;
}
}
}
}
var valid0 = _errs50 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.vm !== undefined){
let data28 = data.vm;
const _errs53 = errors;
if((!(Array.isArray(data28))) && (data28 !== null)){
validate295.errors = [{instancePath:instancePath+"/vm",schemaPath:"#/properties/vm/type",keyword:"type",params:{type: schema121.properties.vm.type},message:"must be array,null"}];
return false;
}
if(errors === _errs53){
if(Array.isArray(data28)){
var valid15 = true;
const len11 = data28.length;
for(let i11=0; i11<len11; i11++){
const _errs55 = errors;
if(!(validate133(data28[i11], {instancePath:instancePath+"/vm/" + i11,parentData:data28,parentDataProperty:i11,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid15 = _errs55 === errors;
if(!valid15){
break;
}
}
}
}
var valid0 = _errs53 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.w !== undefined){
let data30 = data.w;
const _errs56 = errors;
if((!(Array.isArray(data30))) && (data30 !== null)){
validate295.errors = [{instancePath:instancePath+"/w",schemaPath:"#/properties/w/type",keyword:"type",params:{type: schema121.properties.w.type},message:"must be array,null"}];
return false;
}
if(errors === _errs56){
if(Array.isArray(data30)){
var valid16 = true;
const len12 = data30.length;
for(let i12=0; i12<len12; i12++){
const _errs58 = errors;
if(!(validate133(data30[i12], {instancePath:instancePath+"/w/" + i12,parentData:data30,parentDataProperty:i12,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate133.errors : vErrors.concat(validate133.errors);
errors = vErrors.length;
}
var valid16 = _errs58 === errors;
if(!valid16){
break;
}
}
}
}
var valid0 = _errs56 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.wi !== undefined){
let data32 = data.wi;
const _errs59 = errors;
if((!(Array.isArray(data32))) && (data32 !== null)){
validate295.errors = [{instancePath:instancePath+"/wi",schemaPath:"#/properties/wi/type",keyword:"type",params:{type: schema121.properties.wi.type},message:"must be array,null"}];
return false;
}
if(errors === _errs59){
if(Array.isArray(data32)){
var valid17 = true;
const len13 = data32.length;
for(let i13=0; i13<len13; i13++){
const _errs61 = errors;
if(!(validate144(data32[i13], {instancePath:instancePath+"/wi/" + i13,parentData:data32,parentDataProperty:i13,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate144.errors : vErrors.concat(validate144.errors);
errors = vErrors.length;
}
var valid17 = _errs61 === errors;
if(!valid17){
break;
}
}
}
}
var valid0 = _errs59 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.wr !== undefined){
let data34 = data.wr;
const _errs62 = errors;
if((!(Array.isArray(data34))) && (data34 !== null)){
validate295.errors = [{instancePath:instancePath+"/wr",schemaPath:"#/properties/wr/type",keyword:"type",params:{type: schema121.properties.wr.type},message:"must be array,null"}];
return false;
}
if(errors === _errs62){
if(Array.isArray(data34)){
var valid18 = true;
const len14 = data34.length;
for(let i14=0; i14<len14; i14++){
const _errs64 = errors;
if(!(validate144(data34[i14], {instancePath:instancePath+"/wr/" + i14,parentData:data34,parentDataProperty:i14,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate144.errors : vErrors.concat(validate144.errors);
errors = vErrors.length;
}
var valid18 = _errs64 === errors;
if(!valid18){
break;
}
}
}
}
var valid0 = _errs62 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate295.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate295.errors = vErrors;
return errors === 0;
}
validate295.evaluated = {"props":{"branch_constraints":true,"branch_susceptance_formula":true,"constraints":true,"diagnostics":true,"dispatch":true,"flows":true,"formulation":true,"initialization":true,"injections":true,"iterations":true,"limit_rounds":true,"lmp":true,"lmp_q":true,"objective":true,"sensitivities":true,"status":true,"va":true,"vm":true,"w":true,"wi":true,"wr":true},"dynamicProps":false,"dynamicItems":false};


function validate294(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate294.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.left === undefined) && (missing0 = "left")) || ((data.right === undefined) && (missing0 = "right"))){
validate294.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!((((((((key0 === "goal") || (key0 === "improvement")) || (key0 === "left")) || (key0 === "left_value")) || (key0 === "left_view")) || (key0 === "right")) || (key0 === "right_value")) || (key0 === "right_view"))){
validate294.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.goal !== undefined){
let data0 = data.goal;
const _errs2 = errors;
if((typeof data0 !== "string") && (data0 !== null)){
validate294.errors = [{instancePath:instancePath+"/goal",schemaPath:"#/properties/goal/type",keyword:"type",params:{type: schema120.properties.goal.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.improvement !== undefined){
let data1 = data.improvement;
const _errs4 = errors;
if((!((typeof data1 == "number") && (isFinite(data1)))) && (data1 !== null)){
validate294.errors = [{instancePath:instancePath+"/improvement",schemaPath:"#/properties/improvement/type",keyword:"type",params:{type: schema120.properties.improvement.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.left !== undefined){
const _errs6 = errors;
if(typeof data.left !== "string"){
validate294.errors = [{instancePath:instancePath+"/left",schemaPath:"#/properties/left/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.left_value !== undefined){
let data3 = data.left_value;
const _errs8 = errors;
if((!((typeof data3 == "number") && (isFinite(data3)))) && (data3 !== null)){
validate294.errors = [{instancePath:instancePath+"/left_value",schemaPath:"#/properties/left_value/type",keyword:"type",params:{type: schema120.properties.left_value.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.left_view !== undefined){
let data4 = data.left_view;
const _errs10 = errors;
const _errs11 = errors;
let valid1 = false;
const _errs12 = errors;
if(!(validate295(data4, {instancePath:instancePath+"/left_view",parentData:data,parentDataProperty:"left_view",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate295.errors : vErrors.concat(validate295.errors);
errors = vErrors.length;
}
var _valid0 = _errs12 === errors;
valid1 = valid1 || _valid0;
if(_valid0){
var props0 = {};
props0.branch_constraints = true;
props0.branch_susceptance_formula = true;
props0.constraints = true;
props0.diagnostics = true;
props0.dispatch = true;
props0.flows = true;
props0.formulation = true;
props0.initialization = true;
props0.injections = true;
props0.iterations = true;
props0.limit_rounds = true;
props0.lmp = true;
props0.lmp_q = true;
props0.objective = true;
props0.sensitivities = true;
props0.status = true;
props0.va = true;
props0.vm = true;
props0.w = true;
props0.wi = true;
props0.wr = true;
}
const _errs13 = errors;
if(data4 !== null){
const err0 = {instancePath:instancePath+"/left_view",schemaPath:"#/properties/left_view/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs13 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/left_view",schemaPath:"#/properties/left_view/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate294.errors = vErrors;
return false;
}
else {
errors = _errs11;
if(vErrors !== null){
if(_errs11){
vErrors.length = _errs11;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.right !== undefined){
const _errs15 = errors;
if(typeof data.right !== "string"){
validate294.errors = [{instancePath:instancePath+"/right",schemaPath:"#/properties/right/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.right_value !== undefined){
let data6 = data.right_value;
const _errs17 = errors;
if((!((typeof data6 == "number") && (isFinite(data6)))) && (data6 !== null)){
validate294.errors = [{instancePath:instancePath+"/right_value",schemaPath:"#/properties/right_value/type",keyword:"type",params:{type: schema120.properties.right_value.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs17 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.right_view !== undefined){
let data7 = data.right_view;
const _errs19 = errors;
const _errs20 = errors;
let valid2 = false;
const _errs21 = errors;
if(!(validate295(data7, {instancePath:instancePath+"/right_view",parentData:data,parentDataProperty:"right_view",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate295.errors : vErrors.concat(validate295.errors);
errors = vErrors.length;
}
var _valid1 = _errs21 === errors;
valid2 = valid2 || _valid1;
if(_valid1){
var props1 = {};
props1.branch_constraints = true;
props1.branch_susceptance_formula = true;
props1.constraints = true;
props1.diagnostics = true;
props1.dispatch = true;
props1.flows = true;
props1.formulation = true;
props1.initialization = true;
props1.injections = true;
props1.iterations = true;
props1.limit_rounds = true;
props1.lmp = true;
props1.lmp_q = true;
props1.objective = true;
props1.sensitivities = true;
props1.status = true;
props1.va = true;
props1.vm = true;
props1.w = true;
props1.wi = true;
props1.wr = true;
}
const _errs22 = errors;
if(data7 !== null){
const err2 = {instancePath:instancePath+"/right_view",schemaPath:"#/properties/right_view/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs22 === errors;
valid2 = valid2 || _valid1;
if(!valid2){
const err3 = {instancePath:instancePath+"/right_view",schemaPath:"#/properties/right_view/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate294.errors = vErrors;
return false;
}
else {
errors = _errs20;
if(vErrors !== null){
if(_errs20){
vErrors.length = _errs20;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs19 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
else {
validate294.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate294.errors = vErrors;
return errors === 0;
}
validate294.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

const schema122 = {"additionalProperties":false,"properties":{"base_mw":{"format":"double","type":"number"},"bus":{"$ref":"#/$defs/ElementKey"},"delta_mw":{"format":"double","type":"number"},"demand_mw":{"format":"double","type":"number"}},"required":["bus","base_mw","demand_mw","delta_mw"],"type":"object"};

function validate319(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate319.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((data.bus === undefined) && (missing0 = "bus")) || ((data.base_mw === undefined) && (missing0 = "base_mw"))) || ((data.demand_mw === undefined) && (missing0 = "demand_mw"))) || ((data.delta_mw === undefined) && (missing0 = "delta_mw"))){
validate319.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!((((key0 === "base_mw") || (key0 === "bus")) || (key0 === "delta_mw")) || (key0 === "demand_mw"))){
validate319.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.base_mw !== undefined){
let data0 = data.base_mw;
const _errs2 = errors;
if(errors === _errs2){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate319.errors = [{instancePath:instancePath+"/base_mw",schemaPath:"#/properties/base_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.bus !== undefined){
const _errs4 = errors;
if(!(validate266(data.bus, {instancePath:instancePath+"/bus",parentData:data,parentDataProperty:"bus",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate266.errors : vErrors.concat(validate266.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.delta_mw !== undefined){
let data2 = data.delta_mw;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate319.errors = [{instancePath:instancePath+"/delta_mw",schemaPath:"#/properties/delta_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.demand_mw !== undefined){
let data3 = data.demand_mw;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate319.errors = [{instancePath:instancePath+"/demand_mw",schemaPath:"#/properties/demand_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
else {
validate319.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate319.errors = vErrors;
return errors === 0;
}
validate319.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

const schema123 = {"additionalProperties":false,"properties":{"active_goal":{"maxItems":2,"minItems":2,"prefixItems":[{"type":"string"},{"$ref":"#/$defs/GoalRevision"}],"type":["array","null"]},"applied_state":{"type":["string","null"]},"experiment_count":{"format":"uint","minimum":0,"type":"integer"},"id":{"type":"string"},"inspected_state":{"type":["string","null"]},"recent_experiments":{"items":{"$ref":"#/$defs/ExperimentSummary"},"type":"array"},"recommended_state":{"type":["string","null"]},"revision":{"format":"uint64","minimum":0,"type":"integer"},"state_count":{"format":"uint","minimum":0,"type":"integer"},"title":{"type":"string"},"unavailable_historical_states":{"format":"uint","minimum":0,"type":"integer"}},"required":["id","title","revision","state_count","experiment_count","recent_experiments","unavailable_historical_states"],"type":"object"};
const schema124 = {"additionalProperties":false,"properties":{"goal":{"type":["string","null"]},"id":{"type":"string"},"kind":{"$ref":"#/$defs/ExperimentKind"},"rationale":{"type":"string"},"result_states":{"items":{"type":"string"},"type":"array"},"solve_count":{"format":"uint","minimum":0,"type":"integer"},"start_state":{"type":["string","null"]},"termination":{"type":"string"},"trial_count":{"format":"uint","minimum":0,"type":"integer"}},"required":["id","kind","rationale","solve_count","trial_count","result_states","termination"],"type":"object"};

function validate325(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate325.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.id === undefined) && (missing0 = "id")) || ((data.kind === undefined) && (missing0 = "kind"))) || ((data.rationale === undefined) && (missing0 = "rationale"))) || ((data.solve_count === undefined) && (missing0 = "solve_count"))) || ((data.trial_count === undefined) && (missing0 = "trial_count"))) || ((data.result_states === undefined) && (missing0 = "result_states"))) || ((data.termination === undefined) && (missing0 = "termination"))){
validate325.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(func1.call(schema124.properties, key0))){
validate325.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.goal !== undefined){
let data0 = data.goal;
const _errs2 = errors;
if((typeof data0 !== "string") && (data0 !== null)){
validate325.errors = [{instancePath:instancePath+"/goal",schemaPath:"#/properties/goal/type",keyword:"type",params:{type: schema124.properties.goal.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.id !== undefined){
const _errs4 = errors;
if(typeof data.id !== "string"){
validate325.errors = [{instancePath:instancePath+"/id",schemaPath:"#/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.kind !== undefined){
const _errs6 = errors;
if(!(validate256(data.kind, {instancePath:instancePath+"/kind",parentData:data,parentDataProperty:"kind",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate256.errors : vErrors.concat(validate256.errors);
errors = vErrors.length;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rationale !== undefined){
const _errs7 = errors;
if(typeof data.rationale !== "string"){
validate325.errors = [{instancePath:instancePath+"/rationale",schemaPath:"#/properties/rationale/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.result_states !== undefined){
let data4 = data.result_states;
const _errs9 = errors;
if(errors === _errs9){
if(Array.isArray(data4)){
var valid1 = true;
const len0 = data4.length;
for(let i0=0; i0<len0; i0++){
const _errs11 = errors;
if(typeof data4[i0] !== "string"){
validate325.errors = [{instancePath:instancePath+"/result_states/" + i0,schemaPath:"#/properties/result_states/items/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid1 = _errs11 === errors;
if(!valid1){
break;
}
}
}
else {
validate325.errors = [{instancePath:instancePath+"/result_states",schemaPath:"#/properties/result_states/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.solve_count !== undefined){
let data6 = data.solve_count;
const _errs13 = errors;
if(!(((typeof data6 == "number") && (!(data6 % 1) && !isNaN(data6))) && (isFinite(data6)))){
validate325.errors = [{instancePath:instancePath+"/solve_count",schemaPath:"#/properties/solve_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs13){
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 < 0 || isNaN(data6)){
validate325.errors = [{instancePath:instancePath+"/solve_count",schemaPath:"#/properties/solve_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.start_state !== undefined){
let data7 = data.start_state;
const _errs15 = errors;
if((typeof data7 !== "string") && (data7 !== null)){
validate325.errors = [{instancePath:instancePath+"/start_state",schemaPath:"#/properties/start_state/type",keyword:"type",params:{type: schema124.properties.start_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.termination !== undefined){
const _errs17 = errors;
if(typeof data.termination !== "string"){
validate325.errors = [{instancePath:instancePath+"/termination",schemaPath:"#/properties/termination/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs17 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.trial_count !== undefined){
let data9 = data.trial_count;
const _errs19 = errors;
if(!(((typeof data9 == "number") && (!(data9 % 1) && !isNaN(data9))) && (isFinite(data9)))){
validate325.errors = [{instancePath:instancePath+"/trial_count",schemaPath:"#/properties/trial_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs19){
if((typeof data9 == "number") && (isFinite(data9))){
if(data9 < 0 || isNaN(data9)){
validate325.errors = [{instancePath:instancePath+"/trial_count",schemaPath:"#/properties/trial_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs19 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate325.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate325.errors = vErrors;
return errors === 0;
}
validate325.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};


function validate323(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate323.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.id === undefined) && (missing0 = "id")) || ((data.title === undefined) && (missing0 = "title"))) || ((data.revision === undefined) && (missing0 = "revision"))) || ((data.state_count === undefined) && (missing0 = "state_count"))) || ((data.experiment_count === undefined) && (missing0 = "experiment_count"))) || ((data.recent_experiments === undefined) && (missing0 = "recent_experiments"))) || ((data.unavailable_historical_states === undefined) && (missing0 = "unavailable_historical_states"))){
validate323.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(func1.call(schema123.properties, key0))){
validate323.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.active_goal !== undefined){
let data0 = data.active_goal;
const _errs2 = errors;
if((!(Array.isArray(data0))) && (data0 !== null)){
validate323.errors = [{instancePath:instancePath+"/active_goal",schemaPath:"#/properties/active_goal/type",keyword:"type",params:{type: schema123.properties.active_goal.type},message:"must be array,null"}];
return false;
}
if(errors === _errs2){
if(Array.isArray(data0)){
if(data0.length > 2){
validate323.errors = [{instancePath:instancePath+"/active_goal",schemaPath:"#/properties/active_goal/maxItems",keyword:"maxItems",params:{limit: 2},message:"must NOT have more than 2 items"}];
return false;
}
else {
if(data0.length < 2){
validate323.errors = [{instancePath:instancePath+"/active_goal",schemaPath:"#/properties/active_goal/minItems",keyword:"minItems",params:{limit: 2},message:"must NOT have fewer than 2 items"}];
return false;
}
else {
const len0 = data0.length;
if(len0 > 0){
const _errs4 = errors;
if(typeof data0[0] !== "string"){
validate323.errors = [{instancePath:instancePath+"/active_goal/0",schemaPath:"#/properties/active_goal/prefixItems/0/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid1 = _errs4 === errors;
}
if(valid1){
if(len0 > 1){
const _errs6 = errors;
if(!(validate261(data0[1], {instancePath:instancePath+"/active_goal/1",parentData:data0,parentDataProperty:1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate261.errors : vErrors.concat(validate261.errors);
errors = vErrors.length;
}
var valid1 = _errs6 === errors;
}
}
}
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.applied_state !== undefined){
let data3 = data.applied_state;
const _errs7 = errors;
if((typeof data3 !== "string") && (data3 !== null)){
validate323.errors = [{instancePath:instancePath+"/applied_state",schemaPath:"#/properties/applied_state/type",keyword:"type",params:{type: schema123.properties.applied_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.experiment_count !== undefined){
let data4 = data.experiment_count;
const _errs9 = errors;
if(!(((typeof data4 == "number") && (!(data4 % 1) && !isNaN(data4))) && (isFinite(data4)))){
validate323.errors = [{instancePath:instancePath+"/experiment_count",schemaPath:"#/properties/experiment_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs9){
if((typeof data4 == "number") && (isFinite(data4))){
if(data4 < 0 || isNaN(data4)){
validate323.errors = [{instancePath:instancePath+"/experiment_count",schemaPath:"#/properties/experiment_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.id !== undefined){
const _errs11 = errors;
if(typeof data.id !== "string"){
validate323.errors = [{instancePath:instancePath+"/id",schemaPath:"#/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.inspected_state !== undefined){
let data6 = data.inspected_state;
const _errs13 = errors;
if((typeof data6 !== "string") && (data6 !== null)){
validate323.errors = [{instancePath:instancePath+"/inspected_state",schemaPath:"#/properties/inspected_state/type",keyword:"type",params:{type: schema123.properties.inspected_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.recent_experiments !== undefined){
let data7 = data.recent_experiments;
const _errs15 = errors;
if(errors === _errs15){
if(Array.isArray(data7)){
var valid2 = true;
const len1 = data7.length;
for(let i0=0; i0<len1; i0++){
const _errs17 = errors;
if(!(validate325(data7[i0], {instancePath:instancePath+"/recent_experiments/" + i0,parentData:data7,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate325.errors : vErrors.concat(validate325.errors);
errors = vErrors.length;
}
var valid2 = _errs17 === errors;
if(!valid2){
break;
}
}
}
else {
validate323.errors = [{instancePath:instancePath+"/recent_experiments",schemaPath:"#/properties/recent_experiments/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.recommended_state !== undefined){
let data9 = data.recommended_state;
const _errs18 = errors;
if((typeof data9 !== "string") && (data9 !== null)){
validate323.errors = [{instancePath:instancePath+"/recommended_state",schemaPath:"#/properties/recommended_state/type",keyword:"type",params:{type: schema123.properties.recommended_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.revision !== undefined){
let data10 = data.revision;
const _errs20 = errors;
if(!(((typeof data10 == "number") && (!(data10 % 1) && !isNaN(data10))) && (isFinite(data10)))){
validate323.errors = [{instancePath:instancePath+"/revision",schemaPath:"#/properties/revision/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs20){
if((typeof data10 == "number") && (isFinite(data10))){
if(data10 < 0 || isNaN(data10)){
validate323.errors = [{instancePath:instancePath+"/revision",schemaPath:"#/properties/revision/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs20 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.state_count !== undefined){
let data11 = data.state_count;
const _errs22 = errors;
if(!(((typeof data11 == "number") && (!(data11 % 1) && !isNaN(data11))) && (isFinite(data11)))){
validate323.errors = [{instancePath:instancePath+"/state_count",schemaPath:"#/properties/state_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs22){
if((typeof data11 == "number") && (isFinite(data11))){
if(data11 < 0 || isNaN(data11)){
validate323.errors = [{instancePath:instancePath+"/state_count",schemaPath:"#/properties/state_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs22 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.title !== undefined){
const _errs24 = errors;
if(typeof data.title !== "string"){
validate323.errors = [{instancePath:instancePath+"/title",schemaPath:"#/properties/title/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs24 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.unavailable_historical_states !== undefined){
let data13 = data.unavailable_historical_states;
const _errs26 = errors;
if(!(((typeof data13 == "number") && (!(data13 % 1) && !isNaN(data13))) && (isFinite(data13)))){
validate323.errors = [{instancePath:instancePath+"/unavailable_historical_states",schemaPath:"#/properties/unavailable_historical_states/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs26){
if((typeof data13 == "number") && (isFinite(data13))){
if(data13 < 0 || isNaN(data13)){
validate323.errors = [{instancePath:instancePath+"/unavailable_historical_states",schemaPath:"#/properties/unavailable_historical_states/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs26 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate323.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate323.errors = vErrors;
return errors === 0;
}
validate323.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};


function validate401(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate401.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((data.summary === undefined) && (missing0 = "summary")){
validate401.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(((((key0 === "comparison") || (key0 === "demand_changes")) || (key0 === "experiment")) || (key0 === "inspected_view")) || (key0 === "summary"))){
validate401.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.comparison !== undefined){
let data0 = data.comparison;
const _errs2 = errors;
const _errs3 = errors;
let valid1 = false;
const _errs4 = errors;
if(!(validate294(data0, {instancePath:instancePath+"/comparison",parentData:data,parentDataProperty:"comparison",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate294.errors : vErrors.concat(validate294.errors);
errors = vErrors.length;
}
var _valid0 = _errs4 === errors;
valid1 = valid1 || _valid0;
const _errs5 = errors;
if(data0 !== null){
const err0 = {instancePath:instancePath+"/comparison",schemaPath:"#/properties/comparison/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs5 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/comparison",schemaPath:"#/properties/comparison/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate401.errors = vErrors;
return false;
}
else {
errors = _errs3;
if(vErrors !== null){
if(_errs3){
vErrors.length = _errs3;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.demand_changes !== undefined){
let data1 = data.demand_changes;
const _errs7 = errors;
if((!(Array.isArray(data1))) && (data1 !== null)){
validate401.errors = [{instancePath:instancePath+"/demand_changes",schemaPath:"#/properties/demand_changes/type",keyword:"type",params:{type: schema119.properties.demand_changes.type},message:"must be array,null"}];
return false;
}
if(errors === _errs7){
if(Array.isArray(data1)){
var valid2 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
const _errs9 = errors;
if(!(validate319(data1[i0], {instancePath:instancePath+"/demand_changes/" + i0,parentData:data1,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate319.errors : vErrors.concat(validate319.errors);
errors = vErrors.length;
}
var valid2 = _errs9 === errors;
if(!valid2){
break;
}
}
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.experiment !== undefined){
let data3 = data.experiment;
const _errs10 = errors;
if((typeof data3 !== "string") && (data3 !== null)){
validate401.errors = [{instancePath:instancePath+"/experiment",schemaPath:"#/properties/experiment/type",keyword:"type",params:{type: schema119.properties.experiment.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.inspected_view !== undefined){
let data4 = data.inspected_view;
const _errs12 = errors;
const _errs13 = errors;
let valid3 = false;
const _errs14 = errors;
if(!(validate295(data4, {instancePath:instancePath+"/inspected_view",parentData:data,parentDataProperty:"inspected_view",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate295.errors : vErrors.concat(validate295.errors);
errors = vErrors.length;
}
var _valid1 = _errs14 === errors;
valid3 = valid3 || _valid1;
if(_valid1){
var props1 = {};
props1.branch_constraints = true;
props1.branch_susceptance_formula = true;
props1.constraints = true;
props1.diagnostics = true;
props1.dispatch = true;
props1.flows = true;
props1.formulation = true;
props1.initialization = true;
props1.injections = true;
props1.iterations = true;
props1.limit_rounds = true;
props1.lmp = true;
props1.lmp_q = true;
props1.objective = true;
props1.sensitivities = true;
props1.status = true;
props1.va = true;
props1.vm = true;
props1.w = true;
props1.wi = true;
props1.wr = true;
}
const _errs15 = errors;
if(data4 !== null){
const err2 = {instancePath:instancePath+"/inspected_view",schemaPath:"#/properties/inspected_view/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs15 === errors;
valid3 = valid3 || _valid1;
if(!valid3){
const err3 = {instancePath:instancePath+"/inspected_view",schemaPath:"#/properties/inspected_view/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate401.errors = vErrors;
return false;
}
else {
errors = _errs13;
if(vErrors !== null){
if(_errs13){
vErrors.length = _errs13;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.summary !== undefined){
const _errs17 = errors;
if(!(validate323(data.summary, {instancePath:instancePath+"/summary",parentData:data,parentDataProperty:"summary",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate323.errors : vErrors.concat(validate323.errors);
errors = vErrors.length;
}
var valid0 = _errs17 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
else {
validate401.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate401.errors = vErrors;
return errors === 0;
}
validate401.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

export const study_summary = validate406;

function validate406(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate406.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.id === undefined) && (missing0 = "id")) || ((data.title === undefined) && (missing0 = "title"))) || ((data.revision === undefined) && (missing0 = "revision"))) || ((data.state_count === undefined) && (missing0 = "state_count"))) || ((data.experiment_count === undefined) && (missing0 = "experiment_count"))) || ((data.recent_experiments === undefined) && (missing0 = "recent_experiments"))) || ((data.unavailable_historical_states === undefined) && (missing0 = "unavailable_historical_states"))){
validate406.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
const _errs1 = errors;
for(const key0 in data){
if(!(func1.call(schema123.properties, key0))){
validate406.errors = [{instancePath,schemaPath:"#/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"}];
return false;
break;
}
}
if(_errs1 === errors){
if(data.active_goal !== undefined){
let data0 = data.active_goal;
const _errs2 = errors;
if((!(Array.isArray(data0))) && (data0 !== null)){
validate406.errors = [{instancePath:instancePath+"/active_goal",schemaPath:"#/properties/active_goal/type",keyword:"type",params:{type: schema123.properties.active_goal.type},message:"must be array,null"}];
return false;
}
if(errors === _errs2){
if(Array.isArray(data0)){
if(data0.length > 2){
validate406.errors = [{instancePath:instancePath+"/active_goal",schemaPath:"#/properties/active_goal/maxItems",keyword:"maxItems",params:{limit: 2},message:"must NOT have more than 2 items"}];
return false;
}
else {
if(data0.length < 2){
validate406.errors = [{instancePath:instancePath+"/active_goal",schemaPath:"#/properties/active_goal/minItems",keyword:"minItems",params:{limit: 2},message:"must NOT have fewer than 2 items"}];
return false;
}
else {
const len0 = data0.length;
if(len0 > 0){
const _errs4 = errors;
if(typeof data0[0] !== "string"){
validate406.errors = [{instancePath:instancePath+"/active_goal/0",schemaPath:"#/properties/active_goal/prefixItems/0/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid1 = _errs4 === errors;
}
if(valid1){
if(len0 > 1){
const _errs6 = errors;
if(!(validate261(data0[1], {instancePath:instancePath+"/active_goal/1",parentData:data0,parentDataProperty:1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate261.errors : vErrors.concat(validate261.errors);
errors = vErrors.length;
}
var valid1 = _errs6 === errors;
}
}
}
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.applied_state !== undefined){
let data3 = data.applied_state;
const _errs7 = errors;
if((typeof data3 !== "string") && (data3 !== null)){
validate406.errors = [{instancePath:instancePath+"/applied_state",schemaPath:"#/properties/applied_state/type",keyword:"type",params:{type: schema123.properties.applied_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.experiment_count !== undefined){
let data4 = data.experiment_count;
const _errs9 = errors;
if(!(((typeof data4 == "number") && (!(data4 % 1) && !isNaN(data4))) && (isFinite(data4)))){
validate406.errors = [{instancePath:instancePath+"/experiment_count",schemaPath:"#/properties/experiment_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs9){
if((typeof data4 == "number") && (isFinite(data4))){
if(data4 < 0 || isNaN(data4)){
validate406.errors = [{instancePath:instancePath+"/experiment_count",schemaPath:"#/properties/experiment_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.id !== undefined){
const _errs11 = errors;
if(typeof data.id !== "string"){
validate406.errors = [{instancePath:instancePath+"/id",schemaPath:"#/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.inspected_state !== undefined){
let data6 = data.inspected_state;
const _errs13 = errors;
if((typeof data6 !== "string") && (data6 !== null)){
validate406.errors = [{instancePath:instancePath+"/inspected_state",schemaPath:"#/properties/inspected_state/type",keyword:"type",params:{type: schema123.properties.inspected_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.recent_experiments !== undefined){
let data7 = data.recent_experiments;
const _errs15 = errors;
if(errors === _errs15){
if(Array.isArray(data7)){
var valid2 = true;
const len1 = data7.length;
for(let i0=0; i0<len1; i0++){
const _errs17 = errors;
if(!(validate325(data7[i0], {instancePath:instancePath+"/recent_experiments/" + i0,parentData:data7,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate325.errors : vErrors.concat(validate325.errors);
errors = vErrors.length;
}
var valid2 = _errs17 === errors;
if(!valid2){
break;
}
}
}
else {
validate406.errors = [{instancePath:instancePath+"/recent_experiments",schemaPath:"#/properties/recent_experiments/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.recommended_state !== undefined){
let data9 = data.recommended_state;
const _errs18 = errors;
if((typeof data9 !== "string") && (data9 !== null)){
validate406.errors = [{instancePath:instancePath+"/recommended_state",schemaPath:"#/properties/recommended_state/type",keyword:"type",params:{type: schema123.properties.recommended_state.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.revision !== undefined){
let data10 = data.revision;
const _errs20 = errors;
if(!(((typeof data10 == "number") && (!(data10 % 1) && !isNaN(data10))) && (isFinite(data10)))){
validate406.errors = [{instancePath:instancePath+"/revision",schemaPath:"#/properties/revision/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs20){
if((typeof data10 == "number") && (isFinite(data10))){
if(data10 < 0 || isNaN(data10)){
validate406.errors = [{instancePath:instancePath+"/revision",schemaPath:"#/properties/revision/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs20 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.state_count !== undefined){
let data11 = data.state_count;
const _errs22 = errors;
if(!(((typeof data11 == "number") && (!(data11 % 1) && !isNaN(data11))) && (isFinite(data11)))){
validate406.errors = [{instancePath:instancePath+"/state_count",schemaPath:"#/properties/state_count/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs22){
if((typeof data11 == "number") && (isFinite(data11))){
if(data11 < 0 || isNaN(data11)){
validate406.errors = [{instancePath:instancePath+"/state_count",schemaPath:"#/properties/state_count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs22 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.title !== undefined){
const _errs24 = errors;
if(typeof data.title !== "string"){
validate406.errors = [{instancePath:instancePath+"/title",schemaPath:"#/properties/title/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs24 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.unavailable_historical_states !== undefined){
let data13 = data.unavailable_historical_states;
const _errs26 = errors;
if(!(((typeof data13 == "number") && (!(data13 % 1) && !isNaN(data13))) && (isFinite(data13)))){
validate406.errors = [{instancePath:instancePath+"/unavailable_historical_states",schemaPath:"#/properties/unavailable_historical_states/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs26){
if((typeof data13 == "number") && (isFinite(data13))){
if(data13 < 0 || isNaN(data13)){
validate406.errors = [{instancePath:instancePath+"/unavailable_historical_states",schemaPath:"#/properties/unavailable_historical_states/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs26 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate406.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate406.errors = vErrors;
return errors === 0;
}
validate406.evaluated = {"props":true,"dynamicProps":false,"dynamicItems":false};

export const http_saved_case = validate535;
const schema144 = {"properties":{"base_input":{"type":"string"},"display_solution":{"$ref":"#/$defs/SolutionPayload"},"input":{"type":"string"},"solution":{"type":"string"},"view":{"$ref":"#/$defs/SolveOutput"}},"required":["base_input","input","solution","view","display_solution"],"type":"object"};
const schema145 = {"properties":{"branch_constraints":{"items":{"$ref":"#/$defs/BranchConstraintResult"},"type":["array","null"]},"constraints":{"items":{"$ref":"#/$defs/ConstraintResult"},"type":["array","null"]},"dispatch":{"items":{"$ref":"#/$defs/DispatchValue"},"type":"array"},"flows":{"items":{"$ref":"#/$defs/FlowValue"},"type":"array"},"formulation":{"$ref":"#/$defs/Problem"},"initialization":{"anyOf":[{"$ref":"#/$defs/AcPfInitialization"},{"type":"null"}]},"objective":{"format":"double","type":"number"},"objective_kind":{"anyOf":[{"$ref":"#/$defs/ObjectiveKind"},{"type":"null"}],"description":"`cost` when the solved OPF minimized the case's generator cost,\n`feasibility` when the case declares none (`objective` is then zero\nand `prices` is empty)."},"prices":{"items":{"$ref":"#/$defs/PriceValue"},"type":"array"},"status":{"$ref":"#/$defs/SolveStatus"},"va":{"items":{"$ref":"#/$defs/ScalarValue"},"type":"array"},"w":{"items":{"$ref":"#/$defs/ScalarValue"},"type":"array"}},"required":["objective","prices","va","w","flows","dispatch"],"type":"object"};
const schema146 = {"description":"A DC OPF branch's thermal constraint in the final solved program.","properties":{"binding":{"description":"An enforced bound is met to the solver's reporting tolerance.","type":"boolean"},"branch":{"format":"uint","minimum":0,"type":"integer"},"enforced":{"type":"boolean"},"relief_per_mw":{"description":"Objective decrease per additional MW of symmetric rating. Only present\nfor an enforced row with an economic objective.","format":"double","type":["number","null"]}},"required":["branch","enforced","binding"],"type":"object"};

function validate412(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate412.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.branch === undefined) && (missing0 = "branch")) || ((data.enforced === undefined) && (missing0 = "enforced"))) || ((data.binding === undefined) && (missing0 = "binding"))){
validate412.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.binding !== undefined){
const _errs1 = errors;
if(typeof data.binding !== "boolean"){
validate412.errors = [{instancePath:instancePath+"/binding",schemaPath:"#/properties/binding/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.branch !== undefined){
let data1 = data.branch;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate412.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate412.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.enforced !== undefined){
const _errs5 = errors;
if(typeof data.enforced !== "boolean"){
validate412.errors = [{instancePath:instancePath+"/enforced",schemaPath:"#/properties/enforced/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.relief_per_mw !== undefined){
let data3 = data.relief_per_mw;
const _errs7 = errors;
if((!((typeof data3 == "number") && (isFinite(data3)))) && (data3 !== null)){
validate412.errors = [{instancePath:instancePath+"/relief_per_mw",schemaPath:"#/properties/relief_per_mw/type",keyword:"type",params:{type: schema146.properties.relief_per_mw.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate412.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate412.errors = vErrors;
return errors === 0;
}
validate412.evaluated = {"props":{"binding":true,"branch":true,"enforced":true,"relief_per_mw":true},"dynamicProps":false,"dynamicItems":false};

const schema147 = {"description":"The solved state of one [`LinearConstraint`], in request order. `value`,\n`lower`, and `upper` are MW; `shadow_price` is in objective units per MW, the\nsame units as `lmp`.","properties":{"binding":{"description":"`value` sits at a limit, within solver tolerance. Always true for an\nequality.","type":"boolean"},"branches":{"description":"Branches in the resolved row, including flows implied by injection terms.","items":{"format":"uint","minimum":0,"type":"integer"},"type":"array"},"id":{"type":"string"},"lower":{"format":"double","type":["number","null"]},"shadow_price":{"description":"The objective decrease per MW the binding limit is raised:\n`-d objective / d limit`. Positive when the upper limit binds, negative\nwhen the lower limit binds, zero when neither does. A bus's price then\ndiffers from its island's reference bus's by `-shadow_price` times the\nchange in `value` per MW injected at the bus and withdrawn at that reference.\nAbsent when the declared objective gives prices no economic meaning, as\nfor `lmp`.","format":"double","type":["number","null"]},"upper":{"format":"double","type":["number","null"]},"value":{"description":"`sum(coefficient * quantity)` at the solution.","format":"double","type":"number"}},"required":["id","value","binding"],"type":"object"};

function validate414(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate414.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.id === undefined) && (missing0 = "id")) || ((data.value === undefined) && (missing0 = "value"))) || ((data.binding === undefined) && (missing0 = "binding"))){
validate414.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.binding !== undefined){
const _errs1 = errors;
if(typeof data.binding !== "boolean"){
validate414.errors = [{instancePath:instancePath+"/binding",schemaPath:"#/properties/binding/type",keyword:"type",params:{type: "boolean"},message:"must be boolean"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.branches !== undefined){
let data1 = data.branches;
const _errs3 = errors;
if(errors === _errs3){
if(Array.isArray(data1)){
var valid1 = true;
const len0 = data1.length;
for(let i0=0; i0<len0; i0++){
let data2 = data1[i0];
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate414.errors = [{instancePath:instancePath+"/branches/" + i0,schemaPath:"#/properties/branches/items/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate414.errors = [{instancePath:instancePath+"/branches/" + i0,schemaPath:"#/properties/branches/items/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid1 = _errs5 === errors;
if(!valid1){
break;
}
}
}
else {
validate414.errors = [{instancePath:instancePath+"/branches",schemaPath:"#/properties/branches/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.id !== undefined){
const _errs7 = errors;
if(typeof data.id !== "string"){
validate414.errors = [{instancePath:instancePath+"/id",schemaPath:"#/properties/id/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lower !== undefined){
let data4 = data.lower;
const _errs9 = errors;
if((!((typeof data4 == "number") && (isFinite(data4)))) && (data4 !== null)){
validate414.errors = [{instancePath:instancePath+"/lower",schemaPath:"#/properties/lower/type",keyword:"type",params:{type: schema147.properties.lower.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.shadow_price !== undefined){
let data5 = data.shadow_price;
const _errs11 = errors;
if((!((typeof data5 == "number") && (isFinite(data5)))) && (data5 !== null)){
validate414.errors = [{instancePath:instancePath+"/shadow_price",schemaPath:"#/properties/shadow_price/type",keyword:"type",params:{type: schema147.properties.shadow_price.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.upper !== undefined){
let data6 = data.upper;
const _errs13 = errors;
if((!((typeof data6 == "number") && (isFinite(data6)))) && (data6 !== null)){
validate414.errors = [{instancePath:instancePath+"/upper",schemaPath:"#/properties/upper/type",keyword:"type",params:{type: schema147.properties.upper.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data7 = data.value;
const _errs15 = errors;
if(errors === _errs15){
if(!((typeof data7 == "number") && (isFinite(data7)))){
validate414.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate414.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate414.errors = vErrors;
return errors === 0;
}
validate414.evaluated = {"props":{"binding":true,"branches":true,"id":true,"lower":true,"shadow_price":true,"upper":true,"value":true},"dynamicProps":false,"dynamicItems":false};

const schema148 = {"properties":{"bus":{"format":"uint","minimum":0,"type":["integer","null"]},"gen":{"format":"uint","minimum":0,"type":"integer"},"mw":{"format":"double","type":"number"}},"required":["gen","mw"],"type":"object"};

function validate416(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate416.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.gen === undefined) && (missing0 = "gen")) || ((data.mw === undefined) && (missing0 = "mw"))){
validate416.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if((!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))) && (data0 !== null)){
validate416.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: schema148.properties.bus.type},message:"must be integer,null"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate416.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.gen !== undefined){
let data1 = data.gen;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate416.errors = [{instancePath:instancePath+"/gen",schemaPath:"#/properties/gen/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate416.errors = [{instancePath:instancePath+"/gen",schemaPath:"#/properties/gen/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.mw !== undefined){
let data2 = data.mw;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate416.errors = [{instancePath:instancePath+"/mw",schemaPath:"#/properties/mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate416.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate416.errors = vErrors;
return errors === 0;
}
validate416.evaluated = {"props":{"bus":true,"gen":true,"mw":true},"dynamicProps":false,"dynamicItems":false};

const schema149 = {"properties":{"branch":{"format":"uint","minimum":0,"type":"integer"},"loading":{"format":"double","type":"number"},"loading_info":{"anyOf":[{"$ref":"#/$defs/BranchLoading"},{"type":"null"}]},"mw":{"format":"double","type":"number"},"pt":{"format":"double","type":["number","null"]},"qf":{"format":"double","type":["number","null"]},"qt":{"format":"double","type":["number","null"]}},"required":["branch","mw","loading"],"type":"object"};
const schema150 = {"description":"Interpretation of `loading`, independent of economic binding or enforcement.\nAn absent rating means no known equipment limit, even if a numerical solver\nfallback produced a finite utilization. Older responses omit this object.","properties":{"basis":{"$ref":"#/$defs/LoadingBasis"},"limiting_end":{"anyOf":[{"$ref":"#/$defs/LoadingEnd"},{"type":"null"}]},"rating":{"format":"double","type":["number","null"]},"rating_source":{"anyOf":[{"$ref":"#/$defs/RatingSource"},{"type":"null"}]}},"required":["basis"],"type":"object"};
const schema151 = {"description":"Basis of the dimensionless branch utilization, with the rating in MW or MVA.","enum":["mw","mva"],"type":"string"};

function validate420(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate420.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate420.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "mw") || (data === "mva"))){
validate420.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema151.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate420.errors = vErrors;
return errors === 0;
}
validate420.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema152 = {"enum":["from","to","both"],"type":"string"};

function validate422(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate422.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate422.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!(((data === "from") || (data === "to")) || (data === "both"))){
validate422.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema152.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate422.errors = vErrors;
return errors === 0;
}
validate422.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema153 = {"enum":["stated","synthesized"],"type":"string"};

function validate424(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate424.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate424.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "stated") || (data === "synthesized"))){
validate424.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema153.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate424.errors = vErrors;
return errors === 0;
}
validate424.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate419(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate419.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((data.basis === undefined) && (missing0 = "basis")){
validate419.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.basis !== undefined){
const _errs1 = errors;
if(!(validate420(data.basis, {instancePath:instancePath+"/basis",parentData:data,parentDataProperty:"basis",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate420.errors : vErrors.concat(validate420.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.limiting_end !== undefined){
let data1 = data.limiting_end;
const _errs2 = errors;
const _errs3 = errors;
let valid1 = false;
const _errs4 = errors;
if(!(validate422(data1, {instancePath:instancePath+"/limiting_end",parentData:data,parentDataProperty:"limiting_end",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate422.errors : vErrors.concat(validate422.errors);
errors = vErrors.length;
}
var _valid0 = _errs4 === errors;
valid1 = valid1 || _valid0;
const _errs5 = errors;
if(data1 !== null){
const err0 = {instancePath:instancePath+"/limiting_end",schemaPath:"#/properties/limiting_end/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs5 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/limiting_end",schemaPath:"#/properties/limiting_end/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate419.errors = vErrors;
return false;
}
else {
errors = _errs3;
if(vErrors !== null){
if(_errs3){
vErrors.length = _errs3;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rating !== undefined){
let data2 = data.rating;
const _errs7 = errors;
if((!((typeof data2 == "number") && (isFinite(data2)))) && (data2 !== null)){
validate419.errors = [{instancePath:instancePath+"/rating",schemaPath:"#/properties/rating/type",keyword:"type",params:{type: schema150.properties.rating.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rating_source !== undefined){
let data3 = data.rating_source;
const _errs9 = errors;
const _errs10 = errors;
let valid2 = false;
const _errs11 = errors;
if(!(validate424(data3, {instancePath:instancePath+"/rating_source",parentData:data,parentDataProperty:"rating_source",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate424.errors : vErrors.concat(validate424.errors);
errors = vErrors.length;
}
var _valid1 = _errs11 === errors;
valid2 = valid2 || _valid1;
const _errs12 = errors;
if(data3 !== null){
const err2 = {instancePath:instancePath+"/rating_source",schemaPath:"#/properties/rating_source/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs12 === errors;
valid2 = valid2 || _valid1;
if(!valid2){
const err3 = {instancePath:instancePath+"/rating_source",schemaPath:"#/properties/rating_source/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate419.errors = vErrors;
return false;
}
else {
errors = _errs10;
if(vErrors !== null){
if(_errs10){
vErrors.length = _errs10;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate419.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate419.errors = vErrors;
return errors === 0;
}
validate419.evaluated = {"props":{"basis":true,"limiting_end":true,"rating":true,"rating_source":true},"dynamicProps":false,"dynamicItems":false};


function validate418(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate418.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.branch === undefined) && (missing0 = "branch")) || ((data.mw === undefined) && (missing0 = "mw"))) || ((data.loading === undefined) && (missing0 = "loading"))){
validate418.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch !== undefined){
let data0 = data.branch;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate418.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate418.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.loading !== undefined){
let data1 = data.loading;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate418.errors = [{instancePath:instancePath+"/loading",schemaPath:"#/properties/loading/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.loading_info !== undefined){
let data2 = data.loading_info;
const _errs5 = errors;
const _errs6 = errors;
let valid1 = false;
const _errs7 = errors;
if(!(validate419(data2, {instancePath:instancePath+"/loading_info",parentData:data,parentDataProperty:"loading_info",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate419.errors : vErrors.concat(validate419.errors);
errors = vErrors.length;
}
var _valid0 = _errs7 === errors;
valid1 = valid1 || _valid0;
if(_valid0){
var props0 = {};
props0.basis = true;
props0.limiting_end = true;
props0.rating = true;
props0.rating_source = true;
}
const _errs8 = errors;
if(data2 !== null){
const err0 = {instancePath:instancePath+"/loading_info",schemaPath:"#/properties/loading_info/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs8 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/loading_info",schemaPath:"#/properties/loading_info/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate418.errors = vErrors;
return false;
}
else {
errors = _errs6;
if(vErrors !== null){
if(_errs6){
vErrors.length = _errs6;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.mw !== undefined){
let data3 = data.mw;
const _errs10 = errors;
if(errors === _errs10){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate418.errors = [{instancePath:instancePath+"/mw",schemaPath:"#/properties/mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.pt !== undefined){
let data4 = data.pt;
const _errs12 = errors;
if((!((typeof data4 == "number") && (isFinite(data4)))) && (data4 !== null)){
validate418.errors = [{instancePath:instancePath+"/pt",schemaPath:"#/properties/pt/type",keyword:"type",params:{type: schema149.properties.pt.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.qf !== undefined){
let data5 = data.qf;
const _errs14 = errors;
if((!((typeof data5 == "number") && (isFinite(data5)))) && (data5 !== null)){
validate418.errors = [{instancePath:instancePath+"/qf",schemaPath:"#/properties/qf/type",keyword:"type",params:{type: schema149.properties.qf.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs14 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.qt !== undefined){
let data6 = data.qt;
const _errs16 = errors;
if((!((typeof data6 == "number") && (isFinite(data6)))) && (data6 !== null)){
validate418.errors = [{instancePath:instancePath+"/qt",schemaPath:"#/properties/qt/type",keyword:"type",params:{type: schema149.properties.qt.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs16 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate418.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate418.errors = vErrors;
return errors === 0;
}
validate418.evaluated = {"props":{"branch":true,"loading":true,"loading_info":true,"mw":true,"pt":true,"qf":true,"qt":true},"dynamicProps":false,"dynamicItems":false};

const schema154 = {"description":"Which problem to solve. The convex/power flow solve paths, as the lowercase JSON\ntags `\"dcpf\"`/`\"dcopf\"`/`\"acpf\"`/`\"socwr\"`. A plain (not internally tagged) enum\nso a request that omits it defaults to [`DcOpf`](Problem::DcOpf), and `{}` is a\nvalid base-case DC OPF request.\n\nThe `\"acopf\"` tag enables full nonlinear AC OPF only when the opt-in `acopf`\nfeature is built. Shipping adapters do not forward that feature.","oneOf":[{"const":"dcpf","description":"DC power flow: angles and flows at the fixed generator setpoints. No prices,\nno dispatch, no sensitivity.","type":"string"},{"const":"dcopf","description":"DC OPF: the LMP / dispatch / flow workhorse. Differentiable via the DC KKT.","type":"string"},{"const":"acpf","description":"AC polar Newton power flow. Voltages and nodal injections. Differentiable\nvia the AC Newton system.","type":"string"},{"const":"socwr","description":"SOCWR (Jabr) conic relaxation of AC OPF. Differentiable via the conic KKT.","type":"string"},{"const":"acopf","description":"Full nonlinear AC OPF through the opt-in POUNCE backend.","type":"string"}]};

function validate428(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate428.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("dcpf" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "dcpf"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("dcopf" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "dcopf"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
const _errs5 = errors;
if(typeof data !== "string"){
const err4 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if("acpf" !== data){
const err5 = {instancePath,schemaPath:"#/oneOf/2/const",keyword:"const",params:{allowedValue: "acpf"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
}
const _errs7 = errors;
if(typeof data !== "string"){
const err6 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if("socwr" !== data){
const err7 = {instancePath,schemaPath:"#/oneOf/3/const",keyword:"const",params:{allowedValue: "socwr"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
var _valid0 = _errs7 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
}
const _errs9 = errors;
if(typeof data !== "string"){
const err8 = {instancePath,schemaPath:"#/oneOf/4/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
if("acopf" !== data){
const err9 = {instancePath,schemaPath:"#/oneOf/4/const",keyword:"const",params:{allowedValue: "acopf"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
var _valid0 = _errs9 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 4];
}
else {
if(_valid0){
valid0 = true;
passing0 = 4;
}
}
}
}
}
if(!valid0){
const err10 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
validate428.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate428.errors = vErrors;
return errors === 0;
}
validate428.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema155 = {"description":"Successful AC initialization, including any fallback from the requested start.","properties":{"attempts":{"description":"Starts attempted, including the successful one (one-based).","format":"uint","minimum":0,"type":"integer"},"requested":{"$ref":"#/$defs/PowerFlowStart"},"restart":{"description":"Deterministic perturbation number (one-based), only for `perturbed`.","format":"uint","minimum":0,"type":["integer","null"]},"used":{"$ref":"#/$defs/AcPfStartStrategy"}},"required":["requested","used","attempts"],"type":"object"};
const schema156 = {"description":"Where the AC power flow's Newton iteration starts, as the lowercase JSON tags\n`\"flat\"` and `\"case\"`. Only `acpf` iterates from a start; every other\nformulation refuses `\"case\"`.","oneOf":[{"const":"flat","description":"Voltage setpoint magnitudes and every angle at its island's reference angle.","type":"string"},{"const":"case","description":"The case's stored bus voltages: each bus's magnitude and its angle\nrelative to its island's reference bus (star buses of three-winding transformers\nwhere the source carries them), or a typed instance's initial point. A\nstored state that does not converge falls back to the flat start.","type":"string"}]};

function validate431(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate431.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("flat" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "flat"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("case" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "case"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
}
if(!valid0){
const err4 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
validate431.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate431.errors = vErrors;
return errors === 0;
}
validate431.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema157 = {"description":"Initialization that produced a converged AC power flow.","enum":["case","flat","perturbed"],"type":"string"};

function validate433(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate433.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate433.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!(((data === "case") || (data === "flat")) || (data === "perturbed"))){
validate433.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema157.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate433.errors = vErrors;
return errors === 0;
}
validate433.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate430(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate430.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.requested === undefined) && (missing0 = "requested")) || ((data.used === undefined) && (missing0 = "used"))) || ((data.attempts === undefined) && (missing0 = "attempts"))){
validate430.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.attempts !== undefined){
let data0 = data.attempts;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate430.errors = [{instancePath:instancePath+"/attempts",schemaPath:"#/properties/attempts/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate430.errors = [{instancePath:instancePath+"/attempts",schemaPath:"#/properties/attempts/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.requested !== undefined){
const _errs3 = errors;
if(!(validate431(data.requested, {instancePath:instancePath+"/requested",parentData:data,parentDataProperty:"requested",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate431.errors : vErrors.concat(validate431.errors);
errors = vErrors.length;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.restart !== undefined){
let data2 = data.restart;
const _errs4 = errors;
if((!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))) && (data2 !== null)){
validate430.errors = [{instancePath:instancePath+"/restart",schemaPath:"#/properties/restart/type",keyword:"type",params:{type: schema155.properties.restart.type},message:"must be integer,null"}];
return false;
}
if(errors === _errs4){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate430.errors = [{instancePath:instancePath+"/restart",schemaPath:"#/properties/restart/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.used !== undefined){
const _errs6 = errors;
if(!(validate433(data.used, {instancePath:instancePath+"/used",parentData:data,parentDataProperty:"used",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate433.errors : vErrors.concat(validate433.errors);
errors = vErrors.length;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate430.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate430.errors = vErrors;
return errors === 0;
}
validate430.evaluated = {"props":{"attempts":true,"requested":true,"restart":true,"used":true},"dynamicProps":false,"dynamicItems":false};

const schema158 = {"description":"What the solved OPF minimized, as PowerIO declared it for the case. A case\nwith no generator cost data declares a feasibility problem: the reported\n`objective` is then zero by construction and prices and limit multipliers\nare left out, because any feasible point is optimal and they are not unique.","oneOf":[{"const":"cost","description":"Total generator cost, in the case's cost units.","type":"string"},{"const":"feasibility","description":"No cost was declared; the solve found a feasible operating point.","type":"string"}]};

function validate436(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate436.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("cost" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "cost"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("feasibility" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "feasibility"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
}
if(!valid0){
const err4 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
validate436.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate436.errors = vErrors;
return errors === 0;
}
validate436.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema159 = {"description":"The served DC value shapes (the HTTP JSON format). The engine returns the\ncommon `BusScalar` / `BranchFlow` / `GenDispatch`; the server maps\nthose to the DC-specific field names the frontend reads.","properties":{"bus":{"format":"uint","minimum":0,"type":"integer"},"value":{"format":"double","type":"number"}},"required":["bus","value"],"type":"object"};

function validate438(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate438.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.bus === undefined) && (missing0 = "bus")) || ((data.value === undefined) && (missing0 = "value"))){
validate438.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate438.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate438.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data1 = data.value;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate438.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate438.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate438.errors = vErrors;
return errors === 0;
}
validate438.evaluated = {"props":{"bus":true,"value":true},"dynamicProps":false,"dynamicItems":false};

const schema160 = {"description":"A solve outcome that succeeded. A failed solve is the `Err` arm of a solve entry.","oneOf":[{"const":"optimal","description":"An OPF reached optimality.","type":"string"},{"const":"feasible","description":"A power flow or local AC OPF solve reached a validated feasible point.","type":"string"}]};

function validate440(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate440.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("optimal" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "optimal"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("feasible" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "feasible"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
}
if(!valid0){
const err4 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
validate440.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate440.errors = vErrors;
return errors === 0;
}
validate440.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema161 = {"properties":{"bus":{"format":"uint","minimum":0,"type":"integer"},"value":{"format":"double","type":"number"}},"required":["bus","value"],"type":"object"};

function validate442(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate442.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.bus === undefined) && (missing0 = "bus")) || ((data.value === undefined) && (missing0 = "value"))){
validate442.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate442.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate442.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data1 = data.value;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate442.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate442.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate442.errors = vErrors;
return errors === 0;
}
validate442.evaluated = {"props":{"bus":true,"value":true},"dynamicProps":false,"dynamicItems":false};


function validate411(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate411.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((data.objective === undefined) && (missing0 = "objective")) || ((data.prices === undefined) && (missing0 = "prices"))) || ((data.va === undefined) && (missing0 = "va"))) || ((data.w === undefined) && (missing0 = "w"))) || ((data.flows === undefined) && (missing0 = "flows"))) || ((data.dispatch === undefined) && (missing0 = "dispatch"))){
validate411.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch_constraints !== undefined){
let data0 = data.branch_constraints;
const _errs1 = errors;
if((!(Array.isArray(data0))) && (data0 !== null)){
validate411.errors = [{instancePath:instancePath+"/branch_constraints",schemaPath:"#/properties/branch_constraints/type",keyword:"type",params:{type: schema145.properties.branch_constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate412(data0[i0], {instancePath:instancePath+"/branch_constraints/" + i0,parentData:data0,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate412.errors : vErrors.concat(validate412.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.constraints !== undefined){
let data2 = data.constraints;
const _errs4 = errors;
if((!(Array.isArray(data2))) && (data2 !== null)){
validate411.errors = [{instancePath:instancePath+"/constraints",schemaPath:"#/properties/constraints/type",keyword:"type",params:{type: schema145.properties.constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs4){
if(Array.isArray(data2)){
var valid2 = true;
const len1 = data2.length;
for(let i1=0; i1<len1; i1++){
const _errs6 = errors;
if(!(validate414(data2[i1], {instancePath:instancePath+"/constraints/" + i1,parentData:data2,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate414.errors : vErrors.concat(validate414.errors);
errors = vErrors.length;
}
var valid2 = _errs6 === errors;
if(!valid2){
break;
}
}
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.dispatch !== undefined){
let data4 = data.dispatch;
const _errs7 = errors;
if(errors === _errs7){
if(Array.isArray(data4)){
var valid3 = true;
const len2 = data4.length;
for(let i2=0; i2<len2; i2++){
const _errs9 = errors;
if(!(validate416(data4[i2], {instancePath:instancePath+"/dispatch/" + i2,parentData:data4,parentDataProperty:i2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate416.errors : vErrors.concat(validate416.errors);
errors = vErrors.length;
}
var valid3 = _errs9 === errors;
if(!valid3){
break;
}
}
}
else {
validate411.errors = [{instancePath:instancePath+"/dispatch",schemaPath:"#/properties/dispatch/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.flows !== undefined){
let data6 = data.flows;
const _errs10 = errors;
if(errors === _errs10){
if(Array.isArray(data6)){
var valid4 = true;
const len3 = data6.length;
for(let i3=0; i3<len3; i3++){
const _errs12 = errors;
if(!(validate418(data6[i3], {instancePath:instancePath+"/flows/" + i3,parentData:data6,parentDataProperty:i3,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate418.errors : vErrors.concat(validate418.errors);
errors = vErrors.length;
}
var valid4 = _errs12 === errors;
if(!valid4){
break;
}
}
}
else {
validate411.errors = [{instancePath:instancePath+"/flows",schemaPath:"#/properties/flows/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.formulation !== undefined){
const _errs13 = errors;
if(!(validate428(data.formulation, {instancePath:instancePath+"/formulation",parentData:data,parentDataProperty:"formulation",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate428.errors : vErrors.concat(validate428.errors);
errors = vErrors.length;
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.initialization !== undefined){
let data9 = data.initialization;
const _errs14 = errors;
const _errs15 = errors;
let valid5 = false;
const _errs16 = errors;
if(!(validate430(data9, {instancePath:instancePath+"/initialization",parentData:data,parentDataProperty:"initialization",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate430.errors : vErrors.concat(validate430.errors);
errors = vErrors.length;
}
var _valid0 = _errs16 === errors;
valid5 = valid5 || _valid0;
if(_valid0){
var props0 = {};
props0.attempts = true;
props0.requested = true;
props0.restart = true;
props0.used = true;
}
const _errs17 = errors;
if(data9 !== null){
const err0 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs17 === errors;
valid5 = valid5 || _valid0;
if(!valid5){
const err1 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate411.errors = vErrors;
return false;
}
else {
errors = _errs15;
if(vErrors !== null){
if(_errs15){
vErrors.length = _errs15;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs14 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
let data10 = data.objective;
const _errs19 = errors;
if(errors === _errs19){
if(!((typeof data10 == "number") && (isFinite(data10)))){
validate411.errors = [{instancePath:instancePath+"/objective",schemaPath:"#/properties/objective/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs19 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective_kind !== undefined){
let data11 = data.objective_kind;
const _errs21 = errors;
const _errs22 = errors;
let valid6 = false;
const _errs23 = errors;
if(!(validate436(data11, {instancePath:instancePath+"/objective_kind",parentData:data,parentDataProperty:"objective_kind",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate436.errors : vErrors.concat(validate436.errors);
errors = vErrors.length;
}
var _valid1 = _errs23 === errors;
valid6 = valid6 || _valid1;
const _errs24 = errors;
if(data11 !== null){
const err2 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs24 === errors;
valid6 = valid6 || _valid1;
if(!valid6){
const err3 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate411.errors = vErrors;
return false;
}
else {
errors = _errs22;
if(vErrors !== null){
if(_errs22){
vErrors.length = _errs22;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.prices !== undefined){
let data12 = data.prices;
const _errs26 = errors;
if(errors === _errs26){
if(Array.isArray(data12)){
var valid7 = true;
const len4 = data12.length;
for(let i4=0; i4<len4; i4++){
const _errs28 = errors;
if(!(validate438(data12[i4], {instancePath:instancePath+"/prices/" + i4,parentData:data12,parentDataProperty:i4,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate438.errors : vErrors.concat(validate438.errors);
errors = vErrors.length;
}
var valid7 = _errs28 === errors;
if(!valid7){
break;
}
}
}
else {
validate411.errors = [{instancePath:instancePath+"/prices",schemaPath:"#/properties/prices/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs26 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.status !== undefined){
const _errs29 = errors;
if(!(validate440(data.status, {instancePath:instancePath+"/status",parentData:data,parentDataProperty:"status",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate440.errors : vErrors.concat(validate440.errors);
errors = vErrors.length;
}
var valid0 = _errs29 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.va !== undefined){
let data15 = data.va;
const _errs30 = errors;
if(errors === _errs30){
if(Array.isArray(data15)){
var valid8 = true;
const len5 = data15.length;
for(let i5=0; i5<len5; i5++){
const _errs32 = errors;
if(!(validate442(data15[i5], {instancePath:instancePath+"/va/" + i5,parentData:data15,parentDataProperty:i5,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate442.errors : vErrors.concat(validate442.errors);
errors = vErrors.length;
}
var valid8 = _errs32 === errors;
if(!valid8){
break;
}
}
}
else {
validate411.errors = [{instancePath:instancePath+"/va",schemaPath:"#/properties/va/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs30 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.w !== undefined){
let data17 = data.w;
const _errs33 = errors;
if(errors === _errs33){
if(Array.isArray(data17)){
var valid9 = true;
const len6 = data17.length;
for(let i6=0; i6<len6; i6++){
const _errs35 = errors;
if(!(validate442(data17[i6], {instancePath:instancePath+"/w/" + i6,parentData:data17,parentDataProperty:i6,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate442.errors : vErrors.concat(validate442.errors);
errors = vErrors.length;
}
var valid9 = _errs35 === errors;
if(!valid9){
break;
}
}
}
else {
validate411.errors = [{instancePath:instancePath+"/w",schemaPath:"#/properties/w/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs33 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate411.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate411.errors = vErrors;
return errors === 0;
}
validate411.evaluated = {"props":{"branch_constraints":true,"constraints":true,"dispatch":true,"flows":true,"formulation":true,"initialization":true,"objective":true,"objective_kind":true,"prices":true,"status":true,"va":true,"w":true},"dynamicProps":false,"dynamicItems":false};

const schema162 = {"description":"A solve result as the JSON entry points and adapters emit it: every\n[`SolveResponse`] field at the top level, plus facts about the solved\nproblem that the response does not hold.\n\n`objective_kind` is derived from the problem each time a result is emitted,\nnever stored: it says whether the OPF minimized a declared generator cost\nor only looked for a feasible point. It is present for every OPF\nformulation (`dcopf`, `socwr`, `acopf`) and absent for power flows.","properties":{"branch_constraints":{"description":"Thermal constraint state and marginal relief, when exposed by the formulation.","items":{"$ref":"#/$defs/BranchConstraintResult"},"type":["array","null"]},"branch_susceptance_formula":{"anyOf":[{"$ref":"#/$defs/DcFormula"},{"type":"null"}],"description":"Resolved DC convention; absent for AC results and older saved responses."},"constraints":{"description":"The request's linear constraints, in request order (dcopf). Absent when\nthe request stated none.","items":{"$ref":"#/$defs/ConstraintResult"},"type":["array","null"]},"diagnostics":{"description":"How the solve treated the network's islands. Empty, and omitted, when\nevery island is supplied and referenced as stated.","items":{"$ref":"#/$defs/SolveDiagnostic"},"type":"array"},"dispatch":{"items":{"$ref":"#/$defs/GenDispatch"},"type":["array","null"]},"flows":{"items":{"$ref":"#/$defs/BranchFlow"},"type":["array","null"]},"formulation":{"$ref":"#/$defs/Problem","description":"The formulation that produced this, echoed for the client."},"initialization":{"anyOf":[{"$ref":"#/$defs/AcPfInitialization"},{"type":"null"}],"description":"Successful AC power-flow start. Absent for other formulations and legacy results."},"injections":{"description":"Nodal injections (acpf), MW/MVAr.","items":{"$ref":"#/$defs/BusInjection"},"type":["array","null"]},"iterations":{"anyOf":[{"$ref":"#/$defs/Iterations"},{"type":"null"}]},"limit_rounds":{"description":"The solves a DC OPF ran under the request's `limits` (dcopf with\n`limits` only).","items":{"$ref":"#/$defs/LimitRound"},"type":["array","null"]},"lmp":{"description":"Active nodal price when the solved OPF declares a network generator\ncost objective. Feasibility solves leave this absent.","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"lmp_q":{"description":"Reactive nodal price. Always `None` in this build; the field is retained for\nwire-format stability.","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"objective":{"format":"double","type":["number","null"]},"objective_kind":{"anyOf":[{"$ref":"#/$defs/ObjectiveKind"},{"type":"null"}]},"sensitivities":{"description":"One self-describing matrix per requested cell, in request order. Each carries\nits own row/column element ids and the served-unit label.","items":{"$ref":"#/$defs/SensitivityMatrix"},"type":"array"},"status":{"$ref":"#/$defs/SolveStatus"},"va":{"description":"Voltage angle, radians (every path except socwr, which is W-space).","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"vm":{"description":"Voltage magnitude, per unit (ACPF or square root of SOCWR squared magnitude).","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"w":{"description":"Squared voltage magnitude `w = |V|^2`, per unit squared (socwr). The conic\nrelaxation does not guarantee a globally consistent set of voltage angles.","items":{"$ref":"#/$defs/BusScalar"},"type":["array","null"]},"wi":{"items":{"$ref":"#/$defs/BranchScalar"},"type":["array","null"]},"wr":{"description":"Real and imaginary oriented branch voltage products (SOCWR).","items":{"$ref":"#/$defs/BranchScalar"},"type":["array","null"]}},"required":["formulation","status"],"type":"object"};
const schema163 = {"description":"The complete DC convention, including taps and phase shifts.","oneOf":[{"const":"series_susceptance","description":"Legacy Tellegen: series susceptance with shifts, ignoring taps.","type":"string"},{"const":"tap_adjusted_reactance","description":"MATPOWER / PowerModels DCMP: ignore resistance, include taps and shifts.","type":"string"},{"const":"reactance_only","description":"Textbook 1/x, ignoring taps and shifts.","type":"string"},{"const":"tap_adjusted_series_susceptance","description":"Ignore conductance: series susceptance divided by tap, with shifts.","type":"string"},{"const":"series_susceptance_no_phase_shift","description":"PowerModels DCP (PGLib DC baseline): ignore taps and shifts.","type":"string"}]};

function validate448(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate448.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(typeof data !== "string"){
const err0 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
if("series_susceptance" !== data){
const err1 = {instancePath,schemaPath:"#/oneOf/0/const",keyword:"const",params:{allowedValue: "series_susceptance"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
}
const _errs3 = errors;
if(typeof data !== "string"){
const err2 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if("tap_adjusted_reactance" !== data){
const err3 = {instancePath,schemaPath:"#/oneOf/1/const",keyword:"const",params:{allowedValue: "tap_adjusted_reactance"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
var _valid0 = _errs3 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
}
const _errs5 = errors;
if(typeof data !== "string"){
const err4 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
if("reactance_only" !== data){
const err5 = {instancePath,schemaPath:"#/oneOf/2/const",keyword:"const",params:{allowedValue: "reactance_only"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
}
const _errs7 = errors;
if(typeof data !== "string"){
const err6 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if("tap_adjusted_series_susceptance" !== data){
const err7 = {instancePath,schemaPath:"#/oneOf/3/const",keyword:"const",params:{allowedValue: "tap_adjusted_series_susceptance"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
var _valid0 = _errs7 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
}
const _errs9 = errors;
if(typeof data !== "string"){
const err8 = {instancePath,schemaPath:"#/oneOf/4/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
if("series_susceptance_no_phase_shift" !== data){
const err9 = {instancePath,schemaPath:"#/oneOf/4/const",keyword:"const",params:{allowedValue: "series_susceptance_no_phase_shift"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
var _valid0 = _errs9 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 4];
}
else {
if(_valid0){
valid0 = true;
passing0 = 4;
}
}
}
}
}
if(!valid0){
const err10 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
validate448.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate448.errors = vErrors;
return errors === 0;
}
validate448.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema164 = {"description":"A finding about how the solve treated the network, such as an island left out\nbecause nothing supplies it. `code` is stable for programs; `message` is for\npeople. `buses` names the original ids of the buses concerned.","properties":{"buses":{"items":{"format":"uint","minimum":0,"type":"integer"},"type":"array"},"code":{"description":"`island_deenergized`: an island with no in-service generator was left\nout of the solve; its buses carry no results and its load is unserved.\nThe message is PowerIO's finding; for an island an HVDC line fed, it\nnames the lines taken out of service with the island and the power\nthey scheduled into it.\n`island_reference_designated`: a supplied island stated no reference bus,\nso the bus of its largest generator became the island's reference.\n`island_extra_reference`: the AC power flow found several reference\nbuses in one island and kept the first as its slack; the others hold\ntheir voltage magnitude as PV (or PQ) buses.\n`BUILD.HVDC.FIXED_INJECTION`: PowerIO's statement of what the in-service\nHVDC lines inject as fixed transfers at their terminal buses.\n`BUILD.HVDC.TERMINAL_INACTIVE`: an in-service HVDC line has a terminal bus\noutside the solve, so neither end injects; `buses` are its two terminals.\n`settings_ignored`: a reopened Study could not apply some or all of the\ncalculation settings its saved module recorded, such as an option this\nengine does not know; engine defaults stand in for what was dropped.","type":"string"},"message":{"type":"string"}},"required":["code","message","buses"],"type":"object"};

function validate451(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate451.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.code === undefined) && (missing0 = "code")) || ((data.message === undefined) && (missing0 = "message"))) || ((data.buses === undefined) && (missing0 = "buses"))){
validate451.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.buses !== undefined){
let data0 = data.buses;
const _errs1 = errors;
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
let data1 = data0[i0];
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate451.errors = [{instancePath:instancePath+"/buses/" + i0,schemaPath:"#/properties/buses/items/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate451.errors = [{instancePath:instancePath+"/buses/" + i0,schemaPath:"#/properties/buses/items/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
else {
validate451.errors = [{instancePath:instancePath+"/buses",schemaPath:"#/properties/buses/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.code !== undefined){
const _errs5 = errors;
if(typeof data.code !== "string"){
validate451.errors = [{instancePath:instancePath+"/code",schemaPath:"#/properties/code/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.message !== undefined){
const _errs7 = errors;
if(typeof data.message !== "string"){
validate451.errors = [{instancePath:instancePath+"/message",schemaPath:"#/properties/message/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate451.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate451.errors = vErrors;
return errors === 0;
}
validate451.evaluated = {"props":{"buses":true,"code":true,"message":true},"dynamicProps":false,"dynamicItems":false};

const schema165 = {"description":"Generator dispatch, keyed by original generator id. `qg` is `None` on the DC paths.","properties":{"bus":{"description":"Original bus identity, allowing exact aggregation of co-located dispatch.","format":"uint","minimum":0,"type":["integer","null"]},"gen":{"format":"uint","minimum":0,"type":"integer"},"pg":{"format":"double","type":"number"},"qg":{"format":"double","type":["number","null"]}},"required":["gen","pg"],"type":"object"};

function validate453(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate453.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.gen === undefined) && (missing0 = "gen")) || ((data.pg === undefined) && (missing0 = "pg"))){
validate453.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if((!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))) && (data0 !== null)){
validate453.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: schema165.properties.bus.type},message:"must be integer,null"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate453.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.gen !== undefined){
let data1 = data.gen;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate453.errors = [{instancePath:instancePath+"/gen",schemaPath:"#/properties/gen/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate453.errors = [{instancePath:instancePath+"/gen",schemaPath:"#/properties/gen/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.pg !== undefined){
let data2 = data.pg;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate453.errors = [{instancePath:instancePath+"/pg",schemaPath:"#/properties/pg/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.qg !== undefined){
let data3 = data.qg;
const _errs7 = errors;
if((!((typeof data3 == "number") && (isFinite(data3)))) && (data3 !== null)){
validate453.errors = [{instancePath:instancePath+"/qg",schemaPath:"#/properties/qg/type",keyword:"type",params:{type: schema165.properties.qg.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate453.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate453.errors = vErrors;
return errors === 0;
}
validate453.evaluated = {"props":{"bus":true,"gen":true,"pg":true,"qg":true},"dynamicProps":false,"dynamicItems":false};

const schema166 = {"description":"Branch flows, keyed by original branch id (plus the row uid when carried, as in\n[`BusScalar`]). `pf` (from-end active, MW) and `loading` (|S|/limit,\ndimensionless) are present on every formulation that has flows; the reactive\nand to-end legs are `None` on the DC paths.","properties":{"branch":{"format":"uint","minimum":0,"type":"integer"},"loading":{"format":"double","type":"number"},"loading_info":{"anyOf":[{"$ref":"#/$defs/BranchLoading"},{"type":"null"}]},"pf":{"format":"double","type":"number"},"pt":{"format":"double","type":["number","null"]},"qf":{"format":"double","type":["number","null"]},"qt":{"format":"double","type":["number","null"]},"uid":{"type":["string","null"]}},"required":["branch","pf","loading"],"type":"object"};

function validate455(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate455.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.branch === undefined) && (missing0 = "branch")) || ((data.pf === undefined) && (missing0 = "pf"))) || ((data.loading === undefined) && (missing0 = "loading"))){
validate455.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch !== undefined){
let data0 = data.branch;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate455.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate455.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.loading !== undefined){
let data1 = data.loading;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate455.errors = [{instancePath:instancePath+"/loading",schemaPath:"#/properties/loading/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.loading_info !== undefined){
let data2 = data.loading_info;
const _errs5 = errors;
const _errs6 = errors;
let valid1 = false;
const _errs7 = errors;
if(!(validate419(data2, {instancePath:instancePath+"/loading_info",parentData:data,parentDataProperty:"loading_info",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate419.errors : vErrors.concat(validate419.errors);
errors = vErrors.length;
}
var _valid0 = _errs7 === errors;
valid1 = valid1 || _valid0;
if(_valid0){
var props0 = {};
props0.basis = true;
props0.limiting_end = true;
props0.rating = true;
props0.rating_source = true;
}
const _errs8 = errors;
if(data2 !== null){
const err0 = {instancePath:instancePath+"/loading_info",schemaPath:"#/properties/loading_info/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs8 === errors;
valid1 = valid1 || _valid0;
if(!valid1){
const err1 = {instancePath:instancePath+"/loading_info",schemaPath:"#/properties/loading_info/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate455.errors = vErrors;
return false;
}
else {
errors = _errs6;
if(vErrors !== null){
if(_errs6){
vErrors.length = _errs6;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.pf !== undefined){
let data3 = data.pf;
const _errs10 = errors;
if(errors === _errs10){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate455.errors = [{instancePath:instancePath+"/pf",schemaPath:"#/properties/pf/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.pt !== undefined){
let data4 = data.pt;
const _errs12 = errors;
if((!((typeof data4 == "number") && (isFinite(data4)))) && (data4 !== null)){
validate455.errors = [{instancePath:instancePath+"/pt",schemaPath:"#/properties/pt/type",keyword:"type",params:{type: schema166.properties.pt.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.qf !== undefined){
let data5 = data.qf;
const _errs14 = errors;
if((!((typeof data5 == "number") && (isFinite(data5)))) && (data5 !== null)){
validate455.errors = [{instancePath:instancePath+"/qf",schemaPath:"#/properties/qf/type",keyword:"type",params:{type: schema166.properties.qf.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs14 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.qt !== undefined){
let data6 = data.qt;
const _errs16 = errors;
if((!((typeof data6 == "number") && (isFinite(data6)))) && (data6 !== null)){
validate455.errors = [{instancePath:instancePath+"/qt",schemaPath:"#/properties/qt/type",keyword:"type",params:{type: schema166.properties.qt.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs16 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.uid !== undefined){
let data7 = data.uid;
const _errs18 = errors;
if((typeof data7 !== "string") && (data7 !== null)){
validate455.errors = [{instancePath:instancePath+"/uid",schemaPath:"#/properties/uid/type",keyword:"type",params:{type: schema166.properties.uid.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
else {
validate455.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate455.errors = vErrors;
return errors === 0;
}
validate455.evaluated = {"props":{"branch":true,"loading":true,"loading_info":true,"pf":true,"pt":true,"qf":true,"qt":true,"uid":true},"dynamicProps":false,"dynamicItems":false};

const schema167 = {"description":"A nodal net injection (MW / MVAr), keyed by original bus id (plus the row uid\nwhen carried, as in [`BusScalar`]). Injections describe the net nodal power balance.","properties":{"bus":{"format":"uint","minimum":0,"type":"integer"},"p":{"format":"double","type":"number"},"q":{"format":"double","type":"number"},"uid":{"type":["string","null"]}},"required":["bus","p","q"],"type":"object"};

function validate460(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate460.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.bus === undefined) && (missing0 = "bus")) || ((data.p === undefined) && (missing0 = "p"))) || ((data.q === undefined) && (missing0 = "q"))){
validate460.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate460.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate460.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.p !== undefined){
let data1 = data.p;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate460.errors = [{instancePath:instancePath+"/p",schemaPath:"#/properties/p/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.q !== undefined){
let data2 = data.q;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate460.errors = [{instancePath:instancePath+"/q",schemaPath:"#/properties/q/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.uid !== undefined){
let data3 = data.uid;
const _errs7 = errors;
if((typeof data3 !== "string") && (data3 !== null)){
validate460.errors = [{instancePath:instancePath+"/uid",schemaPath:"#/properties/uid/type",keyword:"type",params:{type: schema167.properties.uid.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate460.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate460.errors = vErrors;
return errors === 0;
}
validate460.evaluated = {"props":{"bus":true,"p":true,"q":true,"uid":true},"dynamicProps":false,"dynamicItems":false};

const schema168 = {"anyOf":[{"description":"Interior-point iterate trace (dcopf / socwr).","items":{"$ref":"#/$defs/SolveIteration"},"type":"array"},{"description":"Newton iteration count and final infinity-norm mismatch (acpf).","properties":{"count":{"format":"uint","minimum":0,"type":"integer"},"residual":{"format":"double","type":"number"}},"required":["count","residual"],"type":"object"},{"description":"Nonlinear-program diagnostics. The primal residual is Tellegen's\nindependent maximum P/Q balance mismatch in MW/MVAr; other residuals\nare separated by family below or reported by POUNCE.","properties":{"base_restoration_calls":{"description":"Restoration entries in the original solve, even when a retry wins.","format":"uint","minimum":0,"type":["integer","null"]},"base_solver_status":{"description":"Status before recovery; absent in older responses.","type":["string","null"]},"model_fingerprint":{"type":"string"},"nlp_iterations":{"format":"uint","minimum":0,"type":"integer"},"primal_residual":{"format":"double","type":"number"},"recovered_by":{"type":["string","null"]},"recovery_attempts":{"items":{"type":"string"},"type":"array"},"residuals":{"anyOf":[{"$ref":"#/$defs/AcOpfResiduals"},{"type":"null"}]},"restoration_calls":{"description":"Restoration entries in the selected solve (not the whole ladder).","format":"uint","minimum":0,"type":["integer","null"]},"solver_constraint_violation":{"format":"double","type":"number"},"solver_kkt_error":{"format":"double","type":"number"},"solver_status":{"type":"string"},"total_nlp_iterations":{"description":"Base plus every retry, including rejected attempts.","format":"uint","minimum":0,"type":["integer","null"]}},"required":["nlp_iterations","primal_residual","solver_status","solver_constraint_violation","solver_kkt_error","model_fingerprint"],"type":"object"}],"description":"The convergence record. OPF paths carry the full interior-point trace (for the\nsolve-card sparkline); the AC power flow carries its Newton count and final\nmismatch. Untagged: the OPF arm serializes to the same bare array the DC OPF\nalways returned."};
const schema169 = {"description":"One interior-point iterate: the iteration index, the primal objective, and\nthe primal and dual residuals. Collected once per Clarabel iteration to draw\nthe convergence plot. Shape matches the frontend `SolveIteration`.","properties":{"inf_du":{"format":"double","type":"number"},"inf_pr":{"format":"double","type":"number"},"iter":{"format":"uint32","minimum":0,"type":"integer"},"objective":{"format":"double","type":"number"}},"required":["iter","objective","inf_pr","inf_du"],"type":"object"};

function validate463(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate463.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((data.iter === undefined) && (missing0 = "iter")) || ((data.objective === undefined) && (missing0 = "objective"))) || ((data.inf_pr === undefined) && (missing0 = "inf_pr"))) || ((data.inf_du === undefined) && (missing0 = "inf_du"))){
validate463.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.inf_du !== undefined){
let data0 = data.inf_du;
const _errs1 = errors;
if(errors === _errs1){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate463.errors = [{instancePath:instancePath+"/inf_du",schemaPath:"#/properties/inf_du/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.inf_pr !== undefined){
let data1 = data.inf_pr;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate463.errors = [{instancePath:instancePath+"/inf_pr",schemaPath:"#/properties/inf_pr/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iter !== undefined){
let data2 = data.iter;
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate463.errors = [{instancePath:instancePath+"/iter",schemaPath:"#/properties/iter/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate463.errors = [{instancePath:instancePath+"/iter",schemaPath:"#/properties/iter/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
let data3 = data.objective;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate463.errors = [{instancePath:instancePath+"/objective",schemaPath:"#/properties/objective/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate463.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate463.errors = vErrors;
return errors === 0;
}
validate463.evaluated = {"props":{"inf_du":true,"inf_pr":true,"iter":true,"objective":true},"dynamicProps":false,"dynamicItems":false};

const schema170 = {"description":"Independent AC OPF checks, with units stated per family. Objective agreement\nis separate from primal feasibility. Bounds are evaluated only when selected.","properties":{"active_balance_mw":{"format":"double","type":"number"},"active_generation_bounds_mw":{"format":"double","type":"number"},"angle_limits_rad":{"format":"double","type":"number"},"objective_relative_error":{"format":"double","type":"number"},"piecewise_epigraph_cost":{"format":"double","type":"number"},"reactive_balance_mvar":{"format":"double","type":"number"},"reactive_generation_bounds_mvar":{"format":"double","type":"number"},"reference_angles_rad":{"format":"double","type":"number"},"thermal_limits_mva":{"format":"double","type":"number"},"voltage_bounds_pu":{"format":"double","type":"number"}},"required":["active_balance_mw","reactive_balance_mvar","voltage_bounds_pu","active_generation_bounds_mw","reactive_generation_bounds_mvar","reference_angles_rad","angle_limits_rad","thermal_limits_mva","piecewise_epigraph_cost","objective_relative_error"],"type":"object"};

function validate465(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate465.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((((((data.active_balance_mw === undefined) && (missing0 = "active_balance_mw")) || ((data.reactive_balance_mvar === undefined) && (missing0 = "reactive_balance_mvar"))) || ((data.voltage_bounds_pu === undefined) && (missing0 = "voltage_bounds_pu"))) || ((data.active_generation_bounds_mw === undefined) && (missing0 = "active_generation_bounds_mw"))) || ((data.reactive_generation_bounds_mvar === undefined) && (missing0 = "reactive_generation_bounds_mvar"))) || ((data.reference_angles_rad === undefined) && (missing0 = "reference_angles_rad"))) || ((data.angle_limits_rad === undefined) && (missing0 = "angle_limits_rad"))) || ((data.thermal_limits_mva === undefined) && (missing0 = "thermal_limits_mva"))) || ((data.piecewise_epigraph_cost === undefined) && (missing0 = "piecewise_epigraph_cost"))) || ((data.objective_relative_error === undefined) && (missing0 = "objective_relative_error"))){
validate465.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.active_balance_mw !== undefined){
let data0 = data.active_balance_mw;
const _errs1 = errors;
if(errors === _errs1){
if(!((typeof data0 == "number") && (isFinite(data0)))){
validate465.errors = [{instancePath:instancePath+"/active_balance_mw",schemaPath:"#/properties/active_balance_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.active_generation_bounds_mw !== undefined){
let data1 = data.active_generation_bounds_mw;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate465.errors = [{instancePath:instancePath+"/active_generation_bounds_mw",schemaPath:"#/properties/active_generation_bounds_mw/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.angle_limits_rad !== undefined){
let data2 = data.angle_limits_rad;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate465.errors = [{instancePath:instancePath+"/angle_limits_rad",schemaPath:"#/properties/angle_limits_rad/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective_relative_error !== undefined){
let data3 = data.objective_relative_error;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate465.errors = [{instancePath:instancePath+"/objective_relative_error",schemaPath:"#/properties/objective_relative_error/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.piecewise_epigraph_cost !== undefined){
let data4 = data.piecewise_epigraph_cost;
const _errs9 = errors;
if(errors === _errs9){
if(!((typeof data4 == "number") && (isFinite(data4)))){
validate465.errors = [{instancePath:instancePath+"/piecewise_epigraph_cost",schemaPath:"#/properties/piecewise_epigraph_cost/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.reactive_balance_mvar !== undefined){
let data5 = data.reactive_balance_mvar;
const _errs11 = errors;
if(errors === _errs11){
if(!((typeof data5 == "number") && (isFinite(data5)))){
validate465.errors = [{instancePath:instancePath+"/reactive_balance_mvar",schemaPath:"#/properties/reactive_balance_mvar/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.reactive_generation_bounds_mvar !== undefined){
let data6 = data.reactive_generation_bounds_mvar;
const _errs13 = errors;
if(errors === _errs13){
if(!((typeof data6 == "number") && (isFinite(data6)))){
validate465.errors = [{instancePath:instancePath+"/reactive_generation_bounds_mvar",schemaPath:"#/properties/reactive_generation_bounds_mvar/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.reference_angles_rad !== undefined){
let data7 = data.reference_angles_rad;
const _errs15 = errors;
if(errors === _errs15){
if(!((typeof data7 == "number") && (isFinite(data7)))){
validate465.errors = [{instancePath:instancePath+"/reference_angles_rad",schemaPath:"#/properties/reference_angles_rad/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.thermal_limits_mva !== undefined){
let data8 = data.thermal_limits_mva;
const _errs17 = errors;
if(errors === _errs17){
if(!((typeof data8 == "number") && (isFinite(data8)))){
validate465.errors = [{instancePath:instancePath+"/thermal_limits_mva",schemaPath:"#/properties/thermal_limits_mva/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs17 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.voltage_bounds_pu !== undefined){
let data9 = data.voltage_bounds_pu;
const _errs19 = errors;
if(errors === _errs19){
if(!((typeof data9 == "number") && (isFinite(data9)))){
validate465.errors = [{instancePath:instancePath+"/voltage_bounds_pu",schemaPath:"#/properties/voltage_bounds_pu/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs19 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate465.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate465.errors = vErrors;
return errors === 0;
}
validate465.evaluated = {"props":{"active_balance_mw":true,"active_generation_bounds_mw":true,"angle_limits_rad":true,"objective_relative_error":true,"piecewise_epigraph_cost":true,"reactive_balance_mvar":true,"reactive_generation_bounds_mvar":true,"reference_angles_rad":true,"thermal_limits_mva":true,"voltage_bounds_pu":true},"dynamicProps":false,"dynamicItems":false};


function validate462(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate462.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
const _errs1 = errors;
if(errors === _errs1){
if(Array.isArray(data)){
var valid1 = true;
const len0 = data.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate463(data[i0], {instancePath:instancePath+"/" + i0,parentData:data,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate463.errors : vErrors.concat(validate463.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
else {
const err0 = {instancePath,schemaPath:"#/anyOf/0/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
valid0 = valid0 || _valid0;
if(_valid0){
var items0 = true;
}
const _errs4 = errors;
if(errors === _errs4){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.count === undefined) && (missing0 = "count")) || ((data.residual === undefined) && (missing0 = "residual"))){
const err1 = {instancePath,schemaPath:"#/anyOf/1/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
}
else {
if(data.count !== undefined){
let data1 = data.count;
const _errs6 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
const err2 = {instancePath:instancePath+"/count",schemaPath:"#/anyOf/1/properties/count/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(errors === _errs6){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
const err3 = {instancePath:instancePath+"/count",schemaPath:"#/anyOf/1/properties/count/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
}
var valid2 = _errs6 === errors;
}
else {
var valid2 = true;
}
if(valid2){
if(data.residual !== undefined){
let data2 = data.residual;
const _errs8 = errors;
if(errors === _errs8){
if(!((typeof data2 == "number") && (isFinite(data2)))){
const err4 = {instancePath:instancePath+"/residual",schemaPath:"#/anyOf/1/properties/residual/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
var valid2 = _errs8 === errors;
}
else {
var valid2 = true;
}
}
}
}
else {
const err5 = {instancePath,schemaPath:"#/anyOf/1/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
var _valid0 = _errs4 === errors;
valid0 = valid0 || _valid0;
if(_valid0){
var props0 = {};
props0.count = true;
props0.residual = true;
}
const _errs10 = errors;
if(errors === _errs10){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing1;
if(((((((data.nlp_iterations === undefined) && (missing1 = "nlp_iterations")) || ((data.primal_residual === undefined) && (missing1 = "primal_residual"))) || ((data.solver_status === undefined) && (missing1 = "solver_status"))) || ((data.solver_constraint_violation === undefined) && (missing1 = "solver_constraint_violation"))) || ((data.solver_kkt_error === undefined) && (missing1 = "solver_kkt_error"))) || ((data.model_fingerprint === undefined) && (missing1 = "model_fingerprint"))){
const err6 = {instancePath,schemaPath:"#/anyOf/2/required",keyword:"required",params:{missingProperty: missing1},message:"must have required property '"+missing1+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
else {
if(data.base_restoration_calls !== undefined){
let data3 = data.base_restoration_calls;
const _errs12 = errors;
if((!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))) && (data3 !== null)){
const err7 = {instancePath:instancePath+"/base_restoration_calls",schemaPath:"#/anyOf/2/properties/base_restoration_calls/type",keyword:"type",params:{type: schema168.anyOf[2].properties.base_restoration_calls.type},message:"must be integer,null"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
if(errors === _errs12){
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 0 || isNaN(data3)){
const err8 = {instancePath:instancePath+"/base_restoration_calls",schemaPath:"#/anyOf/2/properties/base_restoration_calls/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
}
var valid3 = _errs12 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.base_solver_status !== undefined){
let data4 = data.base_solver_status;
const _errs14 = errors;
if((typeof data4 !== "string") && (data4 !== null)){
const err9 = {instancePath:instancePath+"/base_solver_status",schemaPath:"#/anyOf/2/properties/base_solver_status/type",keyword:"type",params:{type: schema168.anyOf[2].properties.base_solver_status.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
var valid3 = _errs14 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.model_fingerprint !== undefined){
const _errs16 = errors;
if(typeof data.model_fingerprint !== "string"){
const err10 = {instancePath:instancePath+"/model_fingerprint",schemaPath:"#/anyOf/2/properties/model_fingerprint/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
var valid3 = _errs16 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.nlp_iterations !== undefined){
let data6 = data.nlp_iterations;
const _errs18 = errors;
if(!(((typeof data6 == "number") && (!(data6 % 1) && !isNaN(data6))) && (isFinite(data6)))){
const err11 = {instancePath:instancePath+"/nlp_iterations",schemaPath:"#/anyOf/2/properties/nlp_iterations/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
if(errors === _errs18){
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 < 0 || isNaN(data6)){
const err12 = {instancePath:instancePath+"/nlp_iterations",schemaPath:"#/anyOf/2/properties/nlp_iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
}
}
var valid3 = _errs18 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.primal_residual !== undefined){
let data7 = data.primal_residual;
const _errs20 = errors;
if(errors === _errs20){
if(!((typeof data7 == "number") && (isFinite(data7)))){
const err13 = {instancePath:instancePath+"/primal_residual",schemaPath:"#/anyOf/2/properties/primal_residual/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
var valid3 = _errs20 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.recovered_by !== undefined){
let data8 = data.recovered_by;
const _errs22 = errors;
if((typeof data8 !== "string") && (data8 !== null)){
const err14 = {instancePath:instancePath+"/recovered_by",schemaPath:"#/anyOf/2/properties/recovered_by/type",keyword:"type",params:{type: schema168.anyOf[2].properties.recovered_by.type},message:"must be string,null"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
var valid3 = _errs22 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.recovery_attempts !== undefined){
let data9 = data.recovery_attempts;
const _errs24 = errors;
if(errors === _errs24){
if(Array.isArray(data9)){
var valid4 = true;
const len1 = data9.length;
for(let i1=0; i1<len1; i1++){
const _errs26 = errors;
if(typeof data9[i1] !== "string"){
const err15 = {instancePath:instancePath+"/recovery_attempts/" + i1,schemaPath:"#/anyOf/2/properties/recovery_attempts/items/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
var valid4 = _errs26 === errors;
if(!valid4){
break;
}
}
}
else {
const err16 = {instancePath:instancePath+"/recovery_attempts",schemaPath:"#/anyOf/2/properties/recovery_attempts/type",keyword:"type",params:{type: "array"},message:"must be array"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
}
var valid3 = _errs24 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.residuals !== undefined){
let data11 = data.residuals;
const _errs28 = errors;
const _errs29 = errors;
let valid5 = false;
const _errs30 = errors;
if(!(validate465(data11, {instancePath:instancePath+"/residuals",parentData:data,parentDataProperty:"residuals",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate465.errors : vErrors.concat(validate465.errors);
errors = vErrors.length;
}
var _valid1 = _errs30 === errors;
valid5 = valid5 || _valid1;
if(_valid1){
var props1 = {};
props1.active_balance_mw = true;
props1.active_generation_bounds_mw = true;
props1.angle_limits_rad = true;
props1.objective_relative_error = true;
props1.piecewise_epigraph_cost = true;
props1.reactive_balance_mvar = true;
props1.reactive_generation_bounds_mvar = true;
props1.reference_angles_rad = true;
props1.thermal_limits_mva = true;
props1.voltage_bounds_pu = true;
}
const _errs31 = errors;
if(data11 !== null){
const err17 = {instancePath:instancePath+"/residuals",schemaPath:"#/anyOf/2/properties/residuals/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
var _valid1 = _errs31 === errors;
valid5 = valid5 || _valid1;
if(!valid5){
const err18 = {instancePath:instancePath+"/residuals",schemaPath:"#/anyOf/2/properties/residuals/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
else {
errors = _errs29;
if(vErrors !== null){
if(_errs29){
vErrors.length = _errs29;
}
else {
vErrors = null;
}
}
}
var valid3 = _errs28 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.restoration_calls !== undefined){
let data12 = data.restoration_calls;
const _errs33 = errors;
if((!(((typeof data12 == "number") && (!(data12 % 1) && !isNaN(data12))) && (isFinite(data12)))) && (data12 !== null)){
const err19 = {instancePath:instancePath+"/restoration_calls",schemaPath:"#/anyOf/2/properties/restoration_calls/type",keyword:"type",params:{type: schema168.anyOf[2].properties.restoration_calls.type},message:"must be integer,null"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
if(errors === _errs33){
if((typeof data12 == "number") && (isFinite(data12))){
if(data12 < 0 || isNaN(data12)){
const err20 = {instancePath:instancePath+"/restoration_calls",schemaPath:"#/anyOf/2/properties/restoration_calls/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
}
}
var valid3 = _errs33 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.solver_constraint_violation !== undefined){
let data13 = data.solver_constraint_violation;
const _errs35 = errors;
if(errors === _errs35){
if(!((typeof data13 == "number") && (isFinite(data13)))){
const err21 = {instancePath:instancePath+"/solver_constraint_violation",schemaPath:"#/anyOf/2/properties/solver_constraint_violation/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
}
var valid3 = _errs35 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.solver_kkt_error !== undefined){
let data14 = data.solver_kkt_error;
const _errs37 = errors;
if(errors === _errs37){
if(!((typeof data14 == "number") && (isFinite(data14)))){
const err22 = {instancePath:instancePath+"/solver_kkt_error",schemaPath:"#/anyOf/2/properties/solver_kkt_error/type",keyword:"type",params:{type: "number"},message:"must be number"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
}
var valid3 = _errs37 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.solver_status !== undefined){
const _errs39 = errors;
if(typeof data.solver_status !== "string"){
const err23 = {instancePath:instancePath+"/solver_status",schemaPath:"#/anyOf/2/properties/solver_status/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
}
var valid3 = _errs39 === errors;
}
else {
var valid3 = true;
}
if(valid3){
if(data.total_nlp_iterations !== undefined){
let data16 = data.total_nlp_iterations;
const _errs41 = errors;
if((!(((typeof data16 == "number") && (!(data16 % 1) && !isNaN(data16))) && (isFinite(data16)))) && (data16 !== null)){
const err24 = {instancePath:instancePath+"/total_nlp_iterations",schemaPath:"#/anyOf/2/properties/total_nlp_iterations/type",keyword:"type",params:{type: schema168.anyOf[2].properties.total_nlp_iterations.type},message:"must be integer,null"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
if(errors === _errs41){
if((typeof data16 == "number") && (isFinite(data16))){
if(data16 < 0 || isNaN(data16)){
const err25 = {instancePath:instancePath+"/total_nlp_iterations",schemaPath:"#/anyOf/2/properties/total_nlp_iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
}
}
var valid3 = _errs41 === errors;
}
else {
var valid3 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
const err26 = {instancePath,schemaPath:"#/anyOf/2/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
}
var _valid0 = _errs10 === errors;
valid0 = valid0 || _valid0;
if(_valid0){
if(props0 !== true){
props0 = props0 || {};
props0.base_restoration_calls = true;
props0.base_solver_status = true;
props0.model_fingerprint = true;
props0.nlp_iterations = true;
props0.primal_residual = true;
props0.recovered_by = true;
props0.recovery_attempts = true;
props0.residuals = true;
props0.restoration_calls = true;
props0.solver_constraint_violation = true;
props0.solver_kkt_error = true;
props0.solver_status = true;
props0.total_nlp_iterations = true;
}
}
if(!valid0){
const err27 = {instancePath,schemaPath:"#/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
validate462.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate462.errors = vErrors;
evaluated0.props = props0;
evaluated0.items = items0;
return errors === 0;
}
validate462.evaluated = {"dynamicProps":true,"dynamicItems":true};

const schema171 = {"description":"One solve of a DC OPF run under [`LimitOptions`]: the program it solved and\nhow the solution met the enforceable thermal limits. A run without lazy\nenforcement has one round.","properties":{"added":{"description":"Limits added for the next round: the unenforced ones the flows violate\nor meet, and those loaded at or above `near_binding`.","format":"uint","minimum":0,"type":"integer"},"enforced":{"description":"Thermal limits the round's program enforced.","format":"uint","minimum":0,"type":"integer"},"ipm_iterations":{"description":"Interior-point iterations of the round's solve.","format":"uint","minimum":0,"type":"integer"},"max_loading":{"description":"The largest loading over the enforceable limits, as a fraction.","format":"double","type":"number"},"rows":{"format":"uint","minimum":0,"type":"integer"},"variables":{"description":"Variables and constraint rows of the round's program.","format":"uint","minimum":0,"type":"integer"},"violated":{"description":"Enforceable limits the round's solution violated; zero on the last round.","format":"uint","minimum":0,"type":"integer"}},"required":["enforced","violated","added","max_loading","variables","rows","ipm_iterations"],"type":"object"};

function validate468(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate468.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((data.enforced === undefined) && (missing0 = "enforced")) || ((data.violated === undefined) && (missing0 = "violated"))) || ((data.added === undefined) && (missing0 = "added"))) || ((data.max_loading === undefined) && (missing0 = "max_loading"))) || ((data.variables === undefined) && (missing0 = "variables"))) || ((data.rows === undefined) && (missing0 = "rows"))) || ((data.ipm_iterations === undefined) && (missing0 = "ipm_iterations"))){
validate468.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.added !== undefined){
let data0 = data.added;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate468.errors = [{instancePath:instancePath+"/added",schemaPath:"#/properties/added/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate468.errors = [{instancePath:instancePath+"/added",schemaPath:"#/properties/added/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.enforced !== undefined){
let data1 = data.enforced;
const _errs3 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate468.errors = [{instancePath:instancePath+"/enforced",schemaPath:"#/properties/enforced/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate468.errors = [{instancePath:instancePath+"/enforced",schemaPath:"#/properties/enforced/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.ipm_iterations !== undefined){
let data2 = data.ipm_iterations;
const _errs5 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
validate468.errors = [{instancePath:instancePath+"/ipm_iterations",schemaPath:"#/properties/ipm_iterations/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs5){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
validate468.errors = [{instancePath:instancePath+"/ipm_iterations",schemaPath:"#/properties/ipm_iterations/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.max_loading !== undefined){
let data3 = data.max_loading;
const _errs7 = errors;
if(errors === _errs7){
if(!((typeof data3 == "number") && (isFinite(data3)))){
validate468.errors = [{instancePath:instancePath+"/max_loading",schemaPath:"#/properties/max_loading/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rows !== undefined){
let data4 = data.rows;
const _errs9 = errors;
if(!(((typeof data4 == "number") && (!(data4 % 1) && !isNaN(data4))) && (isFinite(data4)))){
validate468.errors = [{instancePath:instancePath+"/rows",schemaPath:"#/properties/rows/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs9){
if((typeof data4 == "number") && (isFinite(data4))){
if(data4 < 0 || isNaN(data4)){
validate468.errors = [{instancePath:instancePath+"/rows",schemaPath:"#/properties/rows/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.variables !== undefined){
let data5 = data.variables;
const _errs11 = errors;
if(!(((typeof data5 == "number") && (!(data5 % 1) && !isNaN(data5))) && (isFinite(data5)))){
validate468.errors = [{instancePath:instancePath+"/variables",schemaPath:"#/properties/variables/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs11){
if((typeof data5 == "number") && (isFinite(data5))){
if(data5 < 0 || isNaN(data5)){
validate468.errors = [{instancePath:instancePath+"/variables",schemaPath:"#/properties/variables/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.violated !== undefined){
let data6 = data.violated;
const _errs13 = errors;
if(!(((typeof data6 == "number") && (!(data6 % 1) && !isNaN(data6))) && (isFinite(data6)))){
validate468.errors = [{instancePath:instancePath+"/violated",schemaPath:"#/properties/violated/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs13){
if((typeof data6 == "number") && (isFinite(data6))){
if(data6 < 0 || isNaN(data6)){
validate468.errors = [{instancePath:instancePath+"/violated",schemaPath:"#/properties/violated/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate468.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate468.errors = vErrors;
return errors === 0;
}
validate468.evaluated = {"props":{"added":true,"enforced":true,"ipm_iterations":true,"max_loading":true,"rows":true,"variables":true,"violated":true},"dynamicProps":false,"dynamicItems":false};

const schema172 = {"description":"A scalar keyed by original bus id (LMP, voltage, angle, squared magnitude).\n`uid` is the bus's powerio row uid when the solved network carried one, so an\noverlay can re-key on stable identity instead of the positional id.","properties":{"bus":{"format":"uint","minimum":0,"type":"integer"},"uid":{"type":["string","null"]},"value":{"format":"double","type":"number"}},"required":["bus","value"],"type":"object"};

function validate470(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate470.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.bus === undefined) && (missing0 = "bus")) || ((data.value === undefined) && (missing0 = "value"))){
validate470.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate470.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate470.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.uid !== undefined){
let data1 = data.uid;
const _errs3 = errors;
if((typeof data1 !== "string") && (data1 !== null)){
validate470.errors = [{instancePath:instancePath+"/uid",schemaPath:"#/properties/uid/type",keyword:"type",params:{type: schema172.properties.uid.type},message:"must be string,null"}];
return false;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data2 = data.value;
const _errs5 = errors;
if(errors === _errs5){
if(!((typeof data2 == "number") && (isFinite(data2)))){
validate470.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate470.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate470.errors = vErrors;
return errors === 0;
}
validate470.evaluated = {"props":{"bus":true,"uid":true,"value":true},"dynamicProps":false,"dynamicItems":false};

const schema173 = {"description":"A self-describing sensitivity result: `values[r][c] = d(rows[r])/d(cols[c])`,\nwith row and column metadata naming each quantity and its source element, and the\nserved-unit label.","properties":{"cols":{"items":{"$ref":"#/$defs/ColMeta"},"type":"array"},"rows":{"items":{"$ref":"#/$defs/RowMeta"},"type":"array"},"units":{"type":"string"},"values":{"description":"`values[r][c] = d(operand_r)/d(parameter_c)`, in the units named by `units`. The\nengine produces per-unit values; the api edge rescales to served units.","items":{"items":{"format":"double","type":"number"},"type":"array"},"type":"array"}},"required":["values","rows","cols","units"],"type":"object"};
const schema174 = {"description":"Identity and quantity metadata for one column of a [`SensitivityMatrix`].","properties":{"element":{"$ref":"#/$defs/ElementId"},"index":{"description":"Dense index of the element this column varies.","format":"uint","minimum":0,"type":"integer"},"parameter":{"$ref":"#/$defs/Parameter"}},"required":["parameter","element","index"],"type":"object"};
const schema175 = {"description":"A source-id reference for a dense row or column, so a [`SensitivityMatrix`]\nself-describes and the api serializes it without re-deriving id maps.","oneOf":[{"additionalProperties":false,"properties":{"Bus":{"format":"uint","minimum":0,"type":"integer"}},"required":["Bus"],"type":"object"},{"additionalProperties":false,"properties":{"Branch":{"format":"uint","minimum":0,"type":"integer"}},"required":["Branch"],"type":"object"},{"additionalProperties":false,"properties":{"Generator":{"format":"uint","minimum":0,"type":"integer"}},"required":["Generator"],"type":"object"},{"additionalProperties":false,"description":"A caller-supplied linear constraint, by its position in the request.","properties":{"Constraint":{"format":"uint","minimum":0,"type":"integer"}},"required":["Constraint"],"type":"object"}]};

function validate476(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate476.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(errors === _errs1){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((data.Bus === undefined) && (missing0 = "Bus")){
const err0 = {instancePath,schemaPath:"#/oneOf/0/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
else {
const _errs3 = errors;
for(const key0 in data){
if(!(key0 === "Bus")){
const err1 = {instancePath,schemaPath:"#/oneOf/0/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
break;
}
}
if(_errs3 === errors){
if(data.Bus !== undefined){
let data0 = data.Bus;
const _errs4 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
const err2 = {instancePath:instancePath+"/Bus",schemaPath:"#/oneOf/0/properties/Bus/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
if(errors === _errs4){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
const err3 = {instancePath:instancePath+"/Bus",schemaPath:"#/oneOf/0/properties/Bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
}
}
}
}
}
}
else {
const err4 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
var props0 = true;
}
const _errs6 = errors;
if(errors === _errs6){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing1;
if((data.Branch === undefined) && (missing1 = "Branch")){
const err5 = {instancePath,schemaPath:"#/oneOf/1/required",keyword:"required",params:{missingProperty: missing1},message:"must have required property '"+missing1+"'"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
else {
const _errs8 = errors;
for(const key1 in data){
if(!(key1 === "Branch")){
const err6 = {instancePath,schemaPath:"#/oneOf/1/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
break;
}
}
if(_errs8 === errors){
if(data.Branch !== undefined){
let data1 = data.Branch;
const _errs9 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
const err7 = {instancePath:instancePath+"/Branch",schemaPath:"#/oneOf/1/properties/Branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
if(errors === _errs9){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
const err8 = {instancePath:instancePath+"/Branch",schemaPath:"#/oneOf/1/properties/Branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
}
}
}
}
}
}
else {
const err9 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
var _valid0 = _errs6 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
if(props0 !== true){
props0 = true;
}
}
const _errs11 = errors;
if(errors === _errs11){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing2;
if((data.Generator === undefined) && (missing2 = "Generator")){
const err10 = {instancePath,schemaPath:"#/oneOf/2/required",keyword:"required",params:{missingProperty: missing2},message:"must have required property '"+missing2+"'"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
else {
const _errs13 = errors;
for(const key2 in data){
if(!(key2 === "Generator")){
const err11 = {instancePath,schemaPath:"#/oneOf/2/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
break;
}
}
if(_errs13 === errors){
if(data.Generator !== undefined){
let data2 = data.Generator;
const _errs14 = errors;
if(!(((typeof data2 == "number") && (!(data2 % 1) && !isNaN(data2))) && (isFinite(data2)))){
const err12 = {instancePath:instancePath+"/Generator",schemaPath:"#/oneOf/2/properties/Generator/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
}
if(errors === _errs14){
if((typeof data2 == "number") && (isFinite(data2))){
if(data2 < 0 || isNaN(data2)){
const err13 = {instancePath:instancePath+"/Generator",schemaPath:"#/oneOf/2/properties/Generator/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
}
}
}
}
}
else {
const err14 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
}
var _valid0 = _errs11 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
if(props0 !== true){
props0 = true;
}
}
const _errs16 = errors;
if(errors === _errs16){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing3;
if((data.Constraint === undefined) && (missing3 = "Constraint")){
const err15 = {instancePath,schemaPath:"#/oneOf/3/required",keyword:"required",params:{missingProperty: missing3},message:"must have required property '"+missing3+"'"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
}
else {
const _errs18 = errors;
for(const key3 in data){
if(!(key3 === "Constraint")){
const err16 = {instancePath,schemaPath:"#/oneOf/3/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key3},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
break;
}
}
if(_errs18 === errors){
if(data.Constraint !== undefined){
let data3 = data.Constraint;
const _errs19 = errors;
if(!(((typeof data3 == "number") && (!(data3 % 1) && !isNaN(data3))) && (isFinite(data3)))){
const err17 = {instancePath:instancePath+"/Constraint",schemaPath:"#/oneOf/3/properties/Constraint/type",keyword:"type",params:{type: "integer"},message:"must be integer"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
if(errors === _errs19){
if((typeof data3 == "number") && (isFinite(data3))){
if(data3 < 0 || isNaN(data3)){
const err18 = {instancePath:instancePath+"/Constraint",schemaPath:"#/oneOf/3/properties/Constraint/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
}
}
}
}
}
}
}
else {
const err19 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
}
var _valid0 = _errs16 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
if(props0 !== true){
props0 = true;
}
}
}
}
}
if(!valid0){
const err20 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
validate476.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate476.errors = vErrors;
evaluated0.props = props0;
return errors === 0;
}
validate476.evaluated = {"dynamicProps":true,"dynamicItems":false};

const schema176 = {"description":"What a sensitivity is taken *with respect to* — the parameter whose hand-derived\n`dK/dp` column drives the solve.","oneOf":[{"additionalProperties":false,"description":"Bus active/reactive demand `pd` / `qd`, per bus.","properties":{"Demand":{"$ref":"#/$defs/Power"}},"required":["Demand"],"type":"object"},{"additionalProperties":false,"description":"Generation cost coefficient, per generator.","properties":{"Cost":{"$ref":"#/$defs/CostTerm"}},"required":["Cost"],"type":"object"},{"const":"LineLimit","description":"DC thermal limit `fmax` / AC-conic apparent-power `|S|` limit, per branch.","type":"string"},{"additionalProperties":false,"description":"Branch series admittance `g` / `b`, per branch.","properties":{"SeriesAdmittance":{"$ref":"#/$defs/GB"}},"required":["SeriesAdmittance"],"type":"object"},{"additionalProperties":false,"description":"Bus shunt admittance `gs` / `bs`, per bus.","properties":{"ShuntAdmittance":{"$ref":"#/$defs/GB"}},"required":["ShuntAdmittance"],"type":"object"},{"additionalProperties":false,"description":"Voltage-magnitude bound right-hand side (the `w = |V|²` bound), per bus.","properties":{"VoltageBound":{"$ref":"#/$defs/Bound"}},"required":["VoltageBound"],"type":"object"},{"additionalProperties":false,"description":"Generator output bound right-hand side (`pmax` / `pmin` / `qmax` / `qmin`),\nper generator.","properties":{"GenBound":{"properties":{"bound":{"$ref":"#/$defs/Bound"},"power":{"$ref":"#/$defs/Power"}},"required":["power","bound"],"type":"object"}},"required":["GenBound"],"type":"object"},{"additionalProperties":false,"description":"Transformer tap ratio / phase shift, per branch.","properties":{"Transformer":{"$ref":"#/$defs/TapKind"}},"required":["Transformer"],"type":"object"},{"const":"Switching","description":"Branch switching state `sw` in `[0, 1]`, per branch.","type":"string"},{"const":"ConstraintLimit","description":"A shift of both limits of a caller-supplied linear constraint (the\nfixed value of an equality), per constraint in request order.","type":"string"}]};
const schema177 = {"description":"Active or reactive power. The orthogonal P/Q sub-axis shared by prices,\ndispatch, flows, and demand.","enum":["Active","Reactive"],"type":"string"};

function validate479(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate479.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate479.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "Active") || (data === "Reactive"))){
validate479.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema177.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate479.errors = vErrors;
return errors === 0;
}
validate479.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema178 = {"description":"Which generation-cost coefficient: the quadratic `cq` or the linear `cl`.","enum":["Quadratic","Linear"],"type":"string"};

function validate481(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate481.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate481.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "Quadratic") || (data === "Linear"))){
validate481.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema178.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate481.errors = vErrors;
return errors === 0;
}
validate481.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema179 = {"description":"Conductance `g` or susceptance `b`, the real/imaginary parts of an admittance.","enum":["Conductance","Susceptance"],"type":"string"};

function validate483(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate483.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate483.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "Conductance") || (data === "Susceptance"))){
validate483.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema179.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate483.errors = vErrors;
return errors === 0;
}
validate483.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema180 = {"description":"A min or max bound, e.g. the lower/upper voltage-magnitude limit.","enum":["Min","Max"],"type":"string"};

function validate486(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate486.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate486.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "Min") || (data === "Max"))){
validate486.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema180.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate486.errors = vErrors;
return errors === 0;
}
validate486.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema181 = {"description":"A transformer control: the tap ratio or the phase-shift angle.","enum":["Ratio","PhaseShift"],"type":"string"};

function validate490(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate490.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate490.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "Ratio") || (data === "PhaseShift"))){
validate490.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema181.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate490.errors = vErrors;
return errors === 0;
}
validate490.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate478(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate478.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(errors === _errs1){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((data.Demand === undefined) && (missing0 = "Demand")){
const err0 = {instancePath,schemaPath:"#/oneOf/0/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
else {
const _errs3 = errors;
for(const key0 in data){
if(!(key0 === "Demand")){
const err1 = {instancePath,schemaPath:"#/oneOf/0/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
break;
}
}
if(_errs3 === errors){
if(data.Demand !== undefined){
if(!(validate479(data.Demand, {instancePath:instancePath+"/Demand",parentData:data,parentDataProperty:"Demand",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate479.errors : vErrors.concat(validate479.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err2 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
var props0 = true;
}
const _errs5 = errors;
if(errors === _errs5){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing1;
if((data.Cost === undefined) && (missing1 = "Cost")){
const err3 = {instancePath,schemaPath:"#/oneOf/1/required",keyword:"required",params:{missingProperty: missing1},message:"must have required property '"+missing1+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
else {
const _errs7 = errors;
for(const key1 in data){
if(!(key1 === "Cost")){
const err4 = {instancePath,schemaPath:"#/oneOf/1/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
break;
}
}
if(_errs7 === errors){
if(data.Cost !== undefined){
if(!(validate481(data.Cost, {instancePath:instancePath+"/Cost",parentData:data,parentDataProperty:"Cost",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate481.errors : vErrors.concat(validate481.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err5 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
if(props0 !== true){
props0 = true;
}
}
const _errs9 = errors;
if(typeof data !== "string"){
const err6 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
if("LineLimit" !== data){
const err7 = {instancePath,schemaPath:"#/oneOf/2/const",keyword:"const",params:{allowedValue: "LineLimit"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
}
var _valid0 = _errs9 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
}
const _errs11 = errors;
if(errors === _errs11){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing2;
if((data.SeriesAdmittance === undefined) && (missing2 = "SeriesAdmittance")){
const err8 = {instancePath,schemaPath:"#/oneOf/3/required",keyword:"required",params:{missingProperty: missing2},message:"must have required property '"+missing2+"'"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
else {
const _errs13 = errors;
for(const key2 in data){
if(!(key2 === "SeriesAdmittance")){
const err9 = {instancePath,schemaPath:"#/oneOf/3/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
break;
}
}
if(_errs13 === errors){
if(data.SeriesAdmittance !== undefined){
if(!(validate483(data.SeriesAdmittance, {instancePath:instancePath+"/SeriesAdmittance",parentData:data,parentDataProperty:"SeriesAdmittance",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate483.errors : vErrors.concat(validate483.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err10 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
var _valid0 = _errs11 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
if(props0 !== true){
props0 = true;
}
}
const _errs15 = errors;
if(errors === _errs15){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing3;
if((data.ShuntAdmittance === undefined) && (missing3 = "ShuntAdmittance")){
const err11 = {instancePath,schemaPath:"#/oneOf/4/required",keyword:"required",params:{missingProperty: missing3},message:"must have required property '"+missing3+"'"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
else {
const _errs17 = errors;
for(const key3 in data){
if(!(key3 === "ShuntAdmittance")){
const err12 = {instancePath,schemaPath:"#/oneOf/4/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key3},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
break;
}
}
if(_errs17 === errors){
if(data.ShuntAdmittance !== undefined){
if(!(validate483(data.ShuntAdmittance, {instancePath:instancePath+"/ShuntAdmittance",parentData:data,parentDataProperty:"ShuntAdmittance",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate483.errors : vErrors.concat(validate483.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err13 = {instancePath,schemaPath:"#/oneOf/4/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
var _valid0 = _errs15 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 4];
}
else {
if(_valid0){
valid0 = true;
passing0 = 4;
if(props0 !== true){
props0 = true;
}
}
const _errs19 = errors;
if(errors === _errs19){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing4;
if((data.VoltageBound === undefined) && (missing4 = "VoltageBound")){
const err14 = {instancePath,schemaPath:"#/oneOf/5/required",keyword:"required",params:{missingProperty: missing4},message:"must have required property '"+missing4+"'"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
}
else {
const _errs21 = errors;
for(const key4 in data){
if(!(key4 === "VoltageBound")){
const err15 = {instancePath,schemaPath:"#/oneOf/5/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key4},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err15];
}
else {
vErrors.push(err15);
}
errors++;
break;
}
}
if(_errs21 === errors){
if(data.VoltageBound !== undefined){
if(!(validate486(data.VoltageBound, {instancePath:instancePath+"/VoltageBound",parentData:data,parentDataProperty:"VoltageBound",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate486.errors : vErrors.concat(validate486.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err16 = {instancePath,schemaPath:"#/oneOf/5/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err16];
}
else {
vErrors.push(err16);
}
errors++;
}
}
var _valid0 = _errs19 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 5];
}
else {
if(_valid0){
valid0 = true;
passing0 = 5;
if(props0 !== true){
props0 = true;
}
}
const _errs23 = errors;
if(errors === _errs23){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing5;
if((data.GenBound === undefined) && (missing5 = "GenBound")){
const err17 = {instancePath,schemaPath:"#/oneOf/6/required",keyword:"required",params:{missingProperty: missing5},message:"must have required property '"+missing5+"'"};
if(vErrors === null){
vErrors = [err17];
}
else {
vErrors.push(err17);
}
errors++;
}
else {
const _errs25 = errors;
for(const key5 in data){
if(!(key5 === "GenBound")){
const err18 = {instancePath,schemaPath:"#/oneOf/6/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key5},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err18];
}
else {
vErrors.push(err18);
}
errors++;
break;
}
}
if(_errs25 === errors){
if(data.GenBound !== undefined){
let data5 = data.GenBound;
const _errs26 = errors;
if(errors === _errs26){
if(data5 && typeof data5 == "object" && !Array.isArray(data5)){
let missing6;
if(((data5.power === undefined) && (missing6 = "power")) || ((data5.bound === undefined) && (missing6 = "bound"))){
const err19 = {instancePath:instancePath+"/GenBound",schemaPath:"#/oneOf/6/properties/GenBound/required",keyword:"required",params:{missingProperty: missing6},message:"must have required property '"+missing6+"'"};
if(vErrors === null){
vErrors = [err19];
}
else {
vErrors.push(err19);
}
errors++;
}
else {
if(data5.bound !== undefined){
const _errs28 = errors;
if(!(validate486(data5.bound, {instancePath:instancePath+"/GenBound/bound",parentData:data5,parentDataProperty:"bound",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate486.errors : vErrors.concat(validate486.errors);
errors = vErrors.length;
}
var valid7 = _errs28 === errors;
}
else {
var valid7 = true;
}
if(valid7){
if(data5.power !== undefined){
const _errs29 = errors;
if(!(validate479(data5.power, {instancePath:instancePath+"/GenBound/power",parentData:data5,parentDataProperty:"power",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate479.errors : vErrors.concat(validate479.errors);
errors = vErrors.length;
}
var valid7 = _errs29 === errors;
}
else {
var valid7 = true;
}
}
}
}
else {
const err20 = {instancePath:instancePath+"/GenBound",schemaPath:"#/oneOf/6/properties/GenBound/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err20];
}
else {
vErrors.push(err20);
}
errors++;
}
}
}
}
}
}
else {
const err21 = {instancePath,schemaPath:"#/oneOf/6/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err21];
}
else {
vErrors.push(err21);
}
errors++;
}
}
var _valid0 = _errs23 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 6];
}
else {
if(_valid0){
valid0 = true;
passing0 = 6;
if(props0 !== true){
props0 = true;
}
}
const _errs30 = errors;
if(errors === _errs30){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing7;
if((data.Transformer === undefined) && (missing7 = "Transformer")){
const err22 = {instancePath,schemaPath:"#/oneOf/7/required",keyword:"required",params:{missingProperty: missing7},message:"must have required property '"+missing7+"'"};
if(vErrors === null){
vErrors = [err22];
}
else {
vErrors.push(err22);
}
errors++;
}
else {
const _errs32 = errors;
for(const key6 in data){
if(!(key6 === "Transformer")){
const err23 = {instancePath,schemaPath:"#/oneOf/7/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key6},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err23];
}
else {
vErrors.push(err23);
}
errors++;
break;
}
}
if(_errs32 === errors){
if(data.Transformer !== undefined){
if(!(validate490(data.Transformer, {instancePath:instancePath+"/Transformer",parentData:data,parentDataProperty:"Transformer",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate490.errors : vErrors.concat(validate490.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err24 = {instancePath,schemaPath:"#/oneOf/7/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err24];
}
else {
vErrors.push(err24);
}
errors++;
}
}
var _valid0 = _errs30 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 7];
}
else {
if(_valid0){
valid0 = true;
passing0 = 7;
if(props0 !== true){
props0 = true;
}
}
const _errs34 = errors;
if(typeof data !== "string"){
const err25 = {instancePath,schemaPath:"#/oneOf/8/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err25];
}
else {
vErrors.push(err25);
}
errors++;
}
if("Switching" !== data){
const err26 = {instancePath,schemaPath:"#/oneOf/8/const",keyword:"const",params:{allowedValue: "Switching"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err26];
}
else {
vErrors.push(err26);
}
errors++;
}
var _valid0 = _errs34 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 8];
}
else {
if(_valid0){
valid0 = true;
passing0 = 8;
}
const _errs36 = errors;
if(typeof data !== "string"){
const err27 = {instancePath,schemaPath:"#/oneOf/9/type",keyword:"type",params:{type: "string"},message:"must be string"};
if(vErrors === null){
vErrors = [err27];
}
else {
vErrors.push(err27);
}
errors++;
}
if("ConstraintLimit" !== data){
const err28 = {instancePath,schemaPath:"#/oneOf/9/const",keyword:"const",params:{allowedValue: "ConstraintLimit"},message:"must be equal to constant"};
if(vErrors === null){
vErrors = [err28];
}
else {
vErrors.push(err28);
}
errors++;
}
var _valid0 = _errs36 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 9];
}
else {
if(_valid0){
valid0 = true;
passing0 = 9;
}
}
}
}
}
}
}
}
}
}
if(!valid0){
const err29 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err29];
}
else {
vErrors.push(err29);
}
errors++;
validate478.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate478.errors = vErrors;
evaluated0.props = props0;
return errors === 0;
}
validate478.evaluated = {"dynamicProps":true,"dynamicItems":false};


function validate475(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate475.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.parameter === undefined) && (missing0 = "parameter")) || ((data.element === undefined) && (missing0 = "element"))) || ((data.index === undefined) && (missing0 = "index"))){
validate475.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.element !== undefined){
const _errs1 = errors;
if(!(validate476(data.element, {instancePath:instancePath+"/element",parentData:data,parentDataProperty:"element",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate476.errors : vErrors.concat(validate476.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.index !== undefined){
let data1 = data.index;
const _errs2 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate475.errors = [{instancePath:instancePath+"/index",schemaPath:"#/properties/index/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs2){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate475.errors = [{instancePath:instancePath+"/index",schemaPath:"#/properties/index/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.parameter !== undefined){
const _errs4 = errors;
if(!(validate478(data.parameter, {instancePath:instancePath+"/parameter",parentData:data,parentDataProperty:"parameter",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate478.errors : vErrors.concat(validate478.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate475.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate475.errors = vErrors;
return errors === 0;
}
validate475.evaluated = {"props":{"element":true,"index":true,"parameter":true},"dynamicProps":false,"dynamicItems":false};

const schema182 = {"description":"Identity and quantity metadata for one row of a [`SensitivityMatrix`].","properties":{"element":{"$ref":"#/$defs/ElementId"},"index":{"description":"Dense index of the element this row reports.","format":"uint","minimum":0,"type":"integer"},"operand":{"$ref":"#/$defs/Operand"}},"required":["operand","element","index"],"type":"object"};
const schema183 = {"description":"What a sensitivity is taken *of*, named for the physical power quantity rather\nthan a formulation's internal variable. The orthogonal sub-axes (active/reactive,\nfrom/to, voltage representation) are visible to the type system; each formulation\nmaps a request to its own KKT rows or returns [`SensError::Unsupported`].\n\nThe dialects fold into one vocabulary: DC `Angle` is `Voltage(Angle)`, the conic\nsquared magnitude `w` is `Voltage(Squared)`, the nodal price is `Price(Active)`.","oneOf":[{"additionalProperties":false,"description":"Nodal-balance dual (the LMP for active power, the reactive price for\nreactive), per bus.","properties":{"Price":{"$ref":"#/$defs/Power"}},"required":["Price"],"type":"object"},{"additionalProperties":false,"description":"Generator dispatch `pg` / `qg`, per generator.","properties":{"Dispatch":{"$ref":"#/$defs/Power"}},"required":["Dispatch"],"type":"object"},{"additionalProperties":false,"description":"Branch active/reactive flow at the from/to end, per branch.","properties":{"Flow":{"properties":{"end":{"$ref":"#/$defs/End"},"power":{"$ref":"#/$defs/Power"}},"required":["power","end"],"type":"object"}},"required":["Flow"],"type":"object"},{"additionalProperties":false,"description":"Bus voltage in the formulation's representation (`vm` / `va` / `w` / `wr` /\n`wi`), per bus.","properties":{"Voltage":{"$ref":"#/$defs/VoltageKind"}},"required":["Voltage"],"type":"object"}]};
const schema184 = {"description":"Which end of a branch a flow is measured at.","enum":["From","To"],"type":"string"};

function validate499(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate499.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate499.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!((data === "From") || (data === "To"))){
validate499.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema184.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate499.errors = vErrors;
return errors === 0;
}
validate499.evaluated = {"dynamicProps":false,"dynamicItems":false};

const schema185 = {"description":"How a voltage operand is represented: polar (`Magnitude`, `Angle`) for the AC\npower flow, or the W-space lift (`Squared` = `|V|²`, `ProductReal` = `Re(Vi Vj*)`,\n`ProductImag` = `Im(Vi Vj*)`) for the conic relaxation.","enum":["Magnitude","Angle","Squared","ProductReal","ProductImag"],"type":"string"};

function validate502(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate502.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(typeof data !== "string"){
validate502.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
if(!(((((data === "Magnitude") || (data === "Angle")) || (data === "Squared")) || (data === "ProductReal")) || (data === "ProductImag"))){
validate502.errors = [{instancePath,schemaPath:"#/enum",keyword:"enum",params:{allowedValues: schema185.enum},message:"must be equal to one of the allowed values"}];
return false;
}
validate502.errors = vErrors;
return errors === 0;
}
validate502.evaluated = {"dynamicProps":false,"dynamicItems":false};


function validate496(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate496.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
const _errs0 = errors;
let valid0 = false;
let passing0 = null;
const _errs1 = errors;
if(errors === _errs1){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((data.Price === undefined) && (missing0 = "Price")){
const err0 = {instancePath,schemaPath:"#/oneOf/0/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
else {
const _errs3 = errors;
for(const key0 in data){
if(!(key0 === "Price")){
const err1 = {instancePath,schemaPath:"#/oneOf/0/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key0},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
break;
}
}
if(_errs3 === errors){
if(data.Price !== undefined){
if(!(validate479(data.Price, {instancePath:instancePath+"/Price",parentData:data,parentDataProperty:"Price",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate479.errors : vErrors.concat(validate479.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err2 = {instancePath,schemaPath:"#/oneOf/0/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
}
var _valid0 = _errs1 === errors;
if(_valid0){
valid0 = true;
passing0 = 0;
var props0 = true;
}
const _errs5 = errors;
if(errors === _errs5){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing1;
if((data.Dispatch === undefined) && (missing1 = "Dispatch")){
const err3 = {instancePath,schemaPath:"#/oneOf/1/required",keyword:"required",params:{missingProperty: missing1},message:"must have required property '"+missing1+"'"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
}
else {
const _errs7 = errors;
for(const key1 in data){
if(!(key1 === "Dispatch")){
const err4 = {instancePath,schemaPath:"#/oneOf/1/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key1},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
break;
}
}
if(_errs7 === errors){
if(data.Dispatch !== undefined){
if(!(validate479(data.Dispatch, {instancePath:instancePath+"/Dispatch",parentData:data,parentDataProperty:"Dispatch",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate479.errors : vErrors.concat(validate479.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err5 = {instancePath,schemaPath:"#/oneOf/1/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
}
}
var _valid0 = _errs5 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 1];
}
else {
if(_valid0){
valid0 = true;
passing0 = 1;
if(props0 !== true){
props0 = true;
}
}
const _errs9 = errors;
if(errors === _errs9){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing2;
if((data.Flow === undefined) && (missing2 = "Flow")){
const err6 = {instancePath,schemaPath:"#/oneOf/2/required",keyword:"required",params:{missingProperty: missing2},message:"must have required property '"+missing2+"'"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
else {
const _errs11 = errors;
for(const key2 in data){
if(!(key2 === "Flow")){
const err7 = {instancePath,schemaPath:"#/oneOf/2/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key2},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
break;
}
}
if(_errs11 === errors){
if(data.Flow !== undefined){
let data2 = data.Flow;
const _errs12 = errors;
if(errors === _errs12){
if(data2 && typeof data2 == "object" && !Array.isArray(data2)){
let missing3;
if(((data2.power === undefined) && (missing3 = "power")) || ((data2.end === undefined) && (missing3 = "end"))){
const err8 = {instancePath:instancePath+"/Flow",schemaPath:"#/oneOf/2/properties/Flow/required",keyword:"required",params:{missingProperty: missing3},message:"must have required property '"+missing3+"'"};
if(vErrors === null){
vErrors = [err8];
}
else {
vErrors.push(err8);
}
errors++;
}
else {
if(data2.end !== undefined){
const _errs14 = errors;
if(!(validate499(data2.end, {instancePath:instancePath+"/Flow/end",parentData:data2,parentDataProperty:"end",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate499.errors : vErrors.concat(validate499.errors);
errors = vErrors.length;
}
var valid4 = _errs14 === errors;
}
else {
var valid4 = true;
}
if(valid4){
if(data2.power !== undefined){
const _errs15 = errors;
if(!(validate479(data2.power, {instancePath:instancePath+"/Flow/power",parentData:data2,parentDataProperty:"power",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate479.errors : vErrors.concat(validate479.errors);
errors = vErrors.length;
}
var valid4 = _errs15 === errors;
}
else {
var valid4 = true;
}
}
}
}
else {
const err9 = {instancePath:instancePath+"/Flow",schemaPath:"#/oneOf/2/properties/Flow/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err9];
}
else {
vErrors.push(err9);
}
errors++;
}
}
}
}
}
}
else {
const err10 = {instancePath,schemaPath:"#/oneOf/2/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err10];
}
else {
vErrors.push(err10);
}
errors++;
}
}
var _valid0 = _errs9 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 2];
}
else {
if(_valid0){
valid0 = true;
passing0 = 2;
if(props0 !== true){
props0 = true;
}
}
const _errs16 = errors;
if(errors === _errs16){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing4;
if((data.Voltage === undefined) && (missing4 = "Voltage")){
const err11 = {instancePath,schemaPath:"#/oneOf/3/required",keyword:"required",params:{missingProperty: missing4},message:"must have required property '"+missing4+"'"};
if(vErrors === null){
vErrors = [err11];
}
else {
vErrors.push(err11);
}
errors++;
}
else {
const _errs18 = errors;
for(const key3 in data){
if(!(key3 === "Voltage")){
const err12 = {instancePath,schemaPath:"#/oneOf/3/additionalProperties",keyword:"additionalProperties",params:{additionalProperty: key3},message:"must NOT have additional properties"};
if(vErrors === null){
vErrors = [err12];
}
else {
vErrors.push(err12);
}
errors++;
break;
}
}
if(_errs18 === errors){
if(data.Voltage !== undefined){
if(!(validate502(data.Voltage, {instancePath:instancePath+"/Voltage",parentData:data,parentDataProperty:"Voltage",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate502.errors : vErrors.concat(validate502.errors);
errors = vErrors.length;
}
}
}
}
}
else {
const err13 = {instancePath,schemaPath:"#/oneOf/3/type",keyword:"type",params:{type: "object"},message:"must be object"};
if(vErrors === null){
vErrors = [err13];
}
else {
vErrors.push(err13);
}
errors++;
}
}
var _valid0 = _errs16 === errors;
if(_valid0 && valid0){
valid0 = false;
passing0 = [passing0, 3];
}
else {
if(_valid0){
valid0 = true;
passing0 = 3;
if(props0 !== true){
props0 = true;
}
}
}
}
}
if(!valid0){
const err14 = {instancePath,schemaPath:"#/oneOf",keyword:"oneOf",params:{passingSchemas: passing0},message:"must match exactly one schema in oneOf"};
if(vErrors === null){
vErrors = [err14];
}
else {
vErrors.push(err14);
}
errors++;
validate496.errors = vErrors;
return false;
}
else {
errors = _errs0;
if(vErrors !== null){
if(_errs0){
vErrors.length = _errs0;
}
else {
vErrors = null;
}
}
}
validate496.errors = vErrors;
evaluated0.props = props0;
return errors === 0;
}
validate496.evaluated = {"dynamicProps":true,"dynamicItems":false};


function validate494(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate494.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((data.operand === undefined) && (missing0 = "operand")) || ((data.element === undefined) && (missing0 = "element"))) || ((data.index === undefined) && (missing0 = "index"))){
validate494.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.element !== undefined){
const _errs1 = errors;
if(!(validate476(data.element, {instancePath:instancePath+"/element",parentData:data,parentDataProperty:"element",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate476.errors : vErrors.concat(validate476.errors);
errors = vErrors.length;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.index !== undefined){
let data1 = data.index;
const _errs2 = errors;
if(!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))){
validate494.errors = [{instancePath:instancePath+"/index",schemaPath:"#/properties/index/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs2){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate494.errors = [{instancePath:instancePath+"/index",schemaPath:"#/properties/index/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs2 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.operand !== undefined){
const _errs4 = errors;
if(!(validate496(data.operand, {instancePath:instancePath+"/operand",parentData:data,parentDataProperty:"operand",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate496.errors : vErrors.concat(validate496.errors);
errors = vErrors.length;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
else {
validate494.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate494.errors = vErrors;
return errors === 0;
}
validate494.evaluated = {"props":{"element":true,"index":true,"operand":true},"dynamicProps":false,"dynamicItems":false};


function validate474(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate474.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((data.values === undefined) && (missing0 = "values")) || ((data.rows === undefined) && (missing0 = "rows"))) || ((data.cols === undefined) && (missing0 = "cols"))) || ((data.units === undefined) && (missing0 = "units"))){
validate474.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.cols !== undefined){
let data0 = data.cols;
const _errs1 = errors;
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate475(data0[i0], {instancePath:instancePath+"/cols/" + i0,parentData:data0,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate475.errors : vErrors.concat(validate475.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
else {
validate474.errors = [{instancePath:instancePath+"/cols",schemaPath:"#/properties/cols/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.rows !== undefined){
let data2 = data.rows;
const _errs4 = errors;
if(errors === _errs4){
if(Array.isArray(data2)){
var valid2 = true;
const len1 = data2.length;
for(let i1=0; i1<len1; i1++){
const _errs6 = errors;
if(!(validate494(data2[i1], {instancePath:instancePath+"/rows/" + i1,parentData:data2,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate494.errors : vErrors.concat(validate494.errors);
errors = vErrors.length;
}
var valid2 = _errs6 === errors;
if(!valid2){
break;
}
}
}
else {
validate474.errors = [{instancePath:instancePath+"/rows",schemaPath:"#/properties/rows/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.units !== undefined){
const _errs7 = errors;
if(typeof data.units !== "string"){
validate474.errors = [{instancePath:instancePath+"/units",schemaPath:"#/properties/units/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.values !== undefined){
let data5 = data.values;
const _errs9 = errors;
if(errors === _errs9){
if(Array.isArray(data5)){
var valid3 = true;
const len2 = data5.length;
for(let i2=0; i2<len2; i2++){
let data6 = data5[i2];
const _errs11 = errors;
if(errors === _errs11){
if(Array.isArray(data6)){
var valid4 = true;
const len3 = data6.length;
for(let i3=0; i3<len3; i3++){
let data7 = data6[i3];
const _errs13 = errors;
if(errors === _errs13){
if(!((typeof data7 == "number") && (isFinite(data7)))){
validate474.errors = [{instancePath:instancePath+"/values/" + i2+"/" + i3,schemaPath:"#/properties/values/items/items/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid4 = _errs13 === errors;
if(!valid4){
break;
}
}
}
else {
validate474.errors = [{instancePath:instancePath+"/values/" + i2,schemaPath:"#/properties/values/items/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid3 = _errs11 === errors;
if(!valid3){
break;
}
}
}
else {
validate474.errors = [{instancePath:instancePath+"/values",schemaPath:"#/properties/values/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
else {
validate474.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate474.errors = vErrors;
return errors === 0;
}
validate474.evaluated = {"props":{"cols":true,"rows":true,"units":true,"values":true},"dynamicProps":false,"dynamicItems":false};

const schema186 = {"description":"A branch-indexed voltage product, in squared per unit.","properties":{"branch":{"format":"uint","minimum":0,"type":"integer"},"value":{"format":"double","type":"number"}},"required":["branch","value"],"type":"object"};

function validate511(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate511.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.branch === undefined) && (missing0 = "branch")) || ((data.value === undefined) && (missing0 = "value"))){
validate511.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch !== undefined){
let data0 = data.branch;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate511.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate511.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data1 = data.value;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate511.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate511.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate511.errors = vErrors;
return errors === 0;
}
validate511.evaluated = {"props":{"branch":true,"value":true},"dynamicProps":false,"dynamicItems":false};


function validate446(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate446.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.formulation === undefined) && (missing0 = "formulation")) || ((data.status === undefined) && (missing0 = "status"))){
validate446.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch_constraints !== undefined){
let data0 = data.branch_constraints;
const _errs1 = errors;
if((!(Array.isArray(data0))) && (data0 !== null)){
validate446.errors = [{instancePath:instancePath+"/branch_constraints",schemaPath:"#/properties/branch_constraints/type",keyword:"type",params:{type: schema162.properties.branch_constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate412(data0[i0], {instancePath:instancePath+"/branch_constraints/" + i0,parentData:data0,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate412.errors : vErrors.concat(validate412.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.branch_susceptance_formula !== undefined){
let data2 = data.branch_susceptance_formula;
const _errs4 = errors;
const _errs5 = errors;
let valid2 = false;
const _errs6 = errors;
if(!(validate448(data2, {instancePath:instancePath+"/branch_susceptance_formula",parentData:data,parentDataProperty:"branch_susceptance_formula",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate448.errors : vErrors.concat(validate448.errors);
errors = vErrors.length;
}
var _valid0 = _errs6 === errors;
valid2 = valid2 || _valid0;
const _errs7 = errors;
if(data2 !== null){
const err0 = {instancePath:instancePath+"/branch_susceptance_formula",schemaPath:"#/properties/branch_susceptance_formula/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs7 === errors;
valid2 = valid2 || _valid0;
if(!valid2){
const err1 = {instancePath:instancePath+"/branch_susceptance_formula",schemaPath:"#/properties/branch_susceptance_formula/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate446.errors = vErrors;
return false;
}
else {
errors = _errs5;
if(vErrors !== null){
if(_errs5){
vErrors.length = _errs5;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.constraints !== undefined){
let data3 = data.constraints;
const _errs9 = errors;
if((!(Array.isArray(data3))) && (data3 !== null)){
validate446.errors = [{instancePath:instancePath+"/constraints",schemaPath:"#/properties/constraints/type",keyword:"type",params:{type: schema162.properties.constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs9){
if(Array.isArray(data3)){
var valid3 = true;
const len1 = data3.length;
for(let i1=0; i1<len1; i1++){
const _errs11 = errors;
if(!(validate414(data3[i1], {instancePath:instancePath+"/constraints/" + i1,parentData:data3,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate414.errors : vErrors.concat(validate414.errors);
errors = vErrors.length;
}
var valid3 = _errs11 === errors;
if(!valid3){
break;
}
}
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.diagnostics !== undefined){
let data5 = data.diagnostics;
const _errs12 = errors;
if(errors === _errs12){
if(Array.isArray(data5)){
var valid4 = true;
const len2 = data5.length;
for(let i2=0; i2<len2; i2++){
const _errs14 = errors;
if(!(validate451(data5[i2], {instancePath:instancePath+"/diagnostics/" + i2,parentData:data5,parentDataProperty:i2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate451.errors : vErrors.concat(validate451.errors);
errors = vErrors.length;
}
var valid4 = _errs14 === errors;
if(!valid4){
break;
}
}
}
else {
validate446.errors = [{instancePath:instancePath+"/diagnostics",schemaPath:"#/properties/diagnostics/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.dispatch !== undefined){
let data7 = data.dispatch;
const _errs15 = errors;
if((!(Array.isArray(data7))) && (data7 !== null)){
validate446.errors = [{instancePath:instancePath+"/dispatch",schemaPath:"#/properties/dispatch/type",keyword:"type",params:{type: schema162.properties.dispatch.type},message:"must be array,null"}];
return false;
}
if(errors === _errs15){
if(Array.isArray(data7)){
var valid5 = true;
const len3 = data7.length;
for(let i3=0; i3<len3; i3++){
const _errs17 = errors;
if(!(validate453(data7[i3], {instancePath:instancePath+"/dispatch/" + i3,parentData:data7,parentDataProperty:i3,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate453.errors : vErrors.concat(validate453.errors);
errors = vErrors.length;
}
var valid5 = _errs17 === errors;
if(!valid5){
break;
}
}
}
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.flows !== undefined){
let data9 = data.flows;
const _errs18 = errors;
if((!(Array.isArray(data9))) && (data9 !== null)){
validate446.errors = [{instancePath:instancePath+"/flows",schemaPath:"#/properties/flows/type",keyword:"type",params:{type: schema162.properties.flows.type},message:"must be array,null"}];
return false;
}
if(errors === _errs18){
if(Array.isArray(data9)){
var valid6 = true;
const len4 = data9.length;
for(let i4=0; i4<len4; i4++){
const _errs20 = errors;
if(!(validate455(data9[i4], {instancePath:instancePath+"/flows/" + i4,parentData:data9,parentDataProperty:i4,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate455.errors : vErrors.concat(validate455.errors);
errors = vErrors.length;
}
var valid6 = _errs20 === errors;
if(!valid6){
break;
}
}
}
}
var valid0 = _errs18 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.formulation !== undefined){
const _errs21 = errors;
if(!(validate428(data.formulation, {instancePath:instancePath+"/formulation",parentData:data,parentDataProperty:"formulation",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate428.errors : vErrors.concat(validate428.errors);
errors = vErrors.length;
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.initialization !== undefined){
let data12 = data.initialization;
const _errs22 = errors;
const _errs23 = errors;
let valid7 = false;
const _errs24 = errors;
if(!(validate430(data12, {instancePath:instancePath+"/initialization",parentData:data,parentDataProperty:"initialization",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate430.errors : vErrors.concat(validate430.errors);
errors = vErrors.length;
}
var _valid1 = _errs24 === errors;
valid7 = valid7 || _valid1;
if(_valid1){
var props0 = {};
props0.attempts = true;
props0.requested = true;
props0.restart = true;
props0.used = true;
}
const _errs25 = errors;
if(data12 !== null){
const err2 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs25 === errors;
valid7 = valid7 || _valid1;
if(!valid7){
const err3 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate446.errors = vErrors;
return false;
}
else {
errors = _errs23;
if(vErrors !== null){
if(_errs23){
vErrors.length = _errs23;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs22 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.injections !== undefined){
let data13 = data.injections;
const _errs27 = errors;
if((!(Array.isArray(data13))) && (data13 !== null)){
validate446.errors = [{instancePath:instancePath+"/injections",schemaPath:"#/properties/injections/type",keyword:"type",params:{type: schema162.properties.injections.type},message:"must be array,null"}];
return false;
}
if(errors === _errs27){
if(Array.isArray(data13)){
var valid8 = true;
const len5 = data13.length;
for(let i5=0; i5<len5; i5++){
const _errs29 = errors;
if(!(validate460(data13[i5], {instancePath:instancePath+"/injections/" + i5,parentData:data13,parentDataProperty:i5,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate460.errors : vErrors.concat(validate460.errors);
errors = vErrors.length;
}
var valid8 = _errs29 === errors;
if(!valid8){
break;
}
}
}
}
var valid0 = _errs27 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iterations !== undefined){
let data15 = data.iterations;
const _errs30 = errors;
const _errs31 = errors;
let valid9 = false;
const _errs32 = errors;
if(!(validate462(data15, {instancePath:instancePath+"/iterations",parentData:data,parentDataProperty:"iterations",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate462.errors : vErrors.concat(validate462.errors);
errors = vErrors.length;
}
var _valid2 = _errs32 === errors;
valid9 = valid9 || _valid2;
const _errs33 = errors;
if(data15 !== null){
const err4 = {instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err4];
}
else {
vErrors.push(err4);
}
errors++;
}
var _valid2 = _errs33 === errors;
valid9 = valid9 || _valid2;
if(!valid9){
const err5 = {instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err5];
}
else {
vErrors.push(err5);
}
errors++;
validate446.errors = vErrors;
return false;
}
else {
errors = _errs31;
if(vErrors !== null){
if(_errs31){
vErrors.length = _errs31;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs30 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.limit_rounds !== undefined){
let data16 = data.limit_rounds;
const _errs35 = errors;
if((!(Array.isArray(data16))) && (data16 !== null)){
validate446.errors = [{instancePath:instancePath+"/limit_rounds",schemaPath:"#/properties/limit_rounds/type",keyword:"type",params:{type: schema162.properties.limit_rounds.type},message:"must be array,null"}];
return false;
}
if(errors === _errs35){
if(Array.isArray(data16)){
var valid10 = true;
const len6 = data16.length;
for(let i6=0; i6<len6; i6++){
const _errs37 = errors;
if(!(validate468(data16[i6], {instancePath:instancePath+"/limit_rounds/" + i6,parentData:data16,parentDataProperty:i6,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate468.errors : vErrors.concat(validate468.errors);
errors = vErrors.length;
}
var valid10 = _errs37 === errors;
if(!valid10){
break;
}
}
}
}
var valid0 = _errs35 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lmp !== undefined){
let data18 = data.lmp;
const _errs38 = errors;
if((!(Array.isArray(data18))) && (data18 !== null)){
validate446.errors = [{instancePath:instancePath+"/lmp",schemaPath:"#/properties/lmp/type",keyword:"type",params:{type: schema162.properties.lmp.type},message:"must be array,null"}];
return false;
}
if(errors === _errs38){
if(Array.isArray(data18)){
var valid11 = true;
const len7 = data18.length;
for(let i7=0; i7<len7; i7++){
const _errs40 = errors;
if(!(validate470(data18[i7], {instancePath:instancePath+"/lmp/" + i7,parentData:data18,parentDataProperty:i7,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate470.errors : vErrors.concat(validate470.errors);
errors = vErrors.length;
}
var valid11 = _errs40 === errors;
if(!valid11){
break;
}
}
}
}
var valid0 = _errs38 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.lmp_q !== undefined){
let data20 = data.lmp_q;
const _errs41 = errors;
if((!(Array.isArray(data20))) && (data20 !== null)){
validate446.errors = [{instancePath:instancePath+"/lmp_q",schemaPath:"#/properties/lmp_q/type",keyword:"type",params:{type: schema162.properties.lmp_q.type},message:"must be array,null"}];
return false;
}
if(errors === _errs41){
if(Array.isArray(data20)){
var valid12 = true;
const len8 = data20.length;
for(let i8=0; i8<len8; i8++){
const _errs43 = errors;
if(!(validate470(data20[i8], {instancePath:instancePath+"/lmp_q/" + i8,parentData:data20,parentDataProperty:i8,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate470.errors : vErrors.concat(validate470.errors);
errors = vErrors.length;
}
var valid12 = _errs43 === errors;
if(!valid12){
break;
}
}
}
}
var valid0 = _errs41 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
let data22 = data.objective;
const _errs44 = errors;
if((!((typeof data22 == "number") && (isFinite(data22)))) && (data22 !== null)){
validate446.errors = [{instancePath:instancePath+"/objective",schemaPath:"#/properties/objective/type",keyword:"type",params:{type: schema162.properties.objective.type},message:"must be number,null"}];
return false;
}
var valid0 = _errs44 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective_kind !== undefined){
let data23 = data.objective_kind;
const _errs46 = errors;
const _errs47 = errors;
let valid13 = false;
const _errs48 = errors;
if(!(validate436(data23, {instancePath:instancePath+"/objective_kind",parentData:data,parentDataProperty:"objective_kind",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate436.errors : vErrors.concat(validate436.errors);
errors = vErrors.length;
}
var _valid3 = _errs48 === errors;
valid13 = valid13 || _valid3;
const _errs49 = errors;
if(data23 !== null){
const err6 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err6];
}
else {
vErrors.push(err6);
}
errors++;
}
var _valid3 = _errs49 === errors;
valid13 = valid13 || _valid3;
if(!valid13){
const err7 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err7];
}
else {
vErrors.push(err7);
}
errors++;
validate446.errors = vErrors;
return false;
}
else {
errors = _errs47;
if(vErrors !== null){
if(_errs47){
vErrors.length = _errs47;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs46 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.sensitivities !== undefined){
let data24 = data.sensitivities;
const _errs51 = errors;
if(errors === _errs51){
if(Array.isArray(data24)){
var valid14 = true;
const len9 = data24.length;
for(let i9=0; i9<len9; i9++){
const _errs53 = errors;
if(!(validate474(data24[i9], {instancePath:instancePath+"/sensitivities/" + i9,parentData:data24,parentDataProperty:i9,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate474.errors : vErrors.concat(validate474.errors);
errors = vErrors.length;
}
var valid14 = _errs53 === errors;
if(!valid14){
break;
}
}
}
else {
validate446.errors = [{instancePath:instancePath+"/sensitivities",schemaPath:"#/properties/sensitivities/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs51 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.status !== undefined){
const _errs54 = errors;
if(!(validate440(data.status, {instancePath:instancePath+"/status",parentData:data,parentDataProperty:"status",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate440.errors : vErrors.concat(validate440.errors);
errors = vErrors.length;
}
var valid0 = _errs54 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.va !== undefined){
let data27 = data.va;
const _errs55 = errors;
if((!(Array.isArray(data27))) && (data27 !== null)){
validate446.errors = [{instancePath:instancePath+"/va",schemaPath:"#/properties/va/type",keyword:"type",params:{type: schema162.properties.va.type},message:"must be array,null"}];
return false;
}
if(errors === _errs55){
if(Array.isArray(data27)){
var valid15 = true;
const len10 = data27.length;
for(let i10=0; i10<len10; i10++){
const _errs57 = errors;
if(!(validate470(data27[i10], {instancePath:instancePath+"/va/" + i10,parentData:data27,parentDataProperty:i10,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate470.errors : vErrors.concat(validate470.errors);
errors = vErrors.length;
}
var valid15 = _errs57 === errors;
if(!valid15){
break;
}
}
}
}
var valid0 = _errs55 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.vm !== undefined){
let data29 = data.vm;
const _errs58 = errors;
if((!(Array.isArray(data29))) && (data29 !== null)){
validate446.errors = [{instancePath:instancePath+"/vm",schemaPath:"#/properties/vm/type",keyword:"type",params:{type: schema162.properties.vm.type},message:"must be array,null"}];
return false;
}
if(errors === _errs58){
if(Array.isArray(data29)){
var valid16 = true;
const len11 = data29.length;
for(let i11=0; i11<len11; i11++){
const _errs60 = errors;
if(!(validate470(data29[i11], {instancePath:instancePath+"/vm/" + i11,parentData:data29,parentDataProperty:i11,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate470.errors : vErrors.concat(validate470.errors);
errors = vErrors.length;
}
var valid16 = _errs60 === errors;
if(!valid16){
break;
}
}
}
}
var valid0 = _errs58 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.w !== undefined){
let data31 = data.w;
const _errs61 = errors;
if((!(Array.isArray(data31))) && (data31 !== null)){
validate446.errors = [{instancePath:instancePath+"/w",schemaPath:"#/properties/w/type",keyword:"type",params:{type: schema162.properties.w.type},message:"must be array,null"}];
return false;
}
if(errors === _errs61){
if(Array.isArray(data31)){
var valid17 = true;
const len12 = data31.length;
for(let i12=0; i12<len12; i12++){
const _errs63 = errors;
if(!(validate470(data31[i12], {instancePath:instancePath+"/w/" + i12,parentData:data31,parentDataProperty:i12,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate470.errors : vErrors.concat(validate470.errors);
errors = vErrors.length;
}
var valid17 = _errs63 === errors;
if(!valid17){
break;
}
}
}
}
var valid0 = _errs61 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.wi !== undefined){
let data33 = data.wi;
const _errs64 = errors;
if((!(Array.isArray(data33))) && (data33 !== null)){
validate446.errors = [{instancePath:instancePath+"/wi",schemaPath:"#/properties/wi/type",keyword:"type",params:{type: schema162.properties.wi.type},message:"must be array,null"}];
return false;
}
if(errors === _errs64){
if(Array.isArray(data33)){
var valid18 = true;
const len13 = data33.length;
for(let i13=0; i13<len13; i13++){
const _errs66 = errors;
if(!(validate511(data33[i13], {instancePath:instancePath+"/wi/" + i13,parentData:data33,parentDataProperty:i13,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate511.errors : vErrors.concat(validate511.errors);
errors = vErrors.length;
}
var valid18 = _errs66 === errors;
if(!valid18){
break;
}
}
}
}
var valid0 = _errs64 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.wr !== undefined){
let data35 = data.wr;
const _errs67 = errors;
if((!(Array.isArray(data35))) && (data35 !== null)){
validate446.errors = [{instancePath:instancePath+"/wr",schemaPath:"#/properties/wr/type",keyword:"type",params:{type: schema162.properties.wr.type},message:"must be array,null"}];
return false;
}
if(errors === _errs67){
if(Array.isArray(data35)){
var valid19 = true;
const len14 = data35.length;
for(let i14=0; i14<len14; i14++){
const _errs69 = errors;
if(!(validate511(data35[i14], {instancePath:instancePath+"/wr/" + i14,parentData:data35,parentDataProperty:i14,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate511.errors : vErrors.concat(validate511.errors);
errors = vErrors.length;
}
var valid19 = _errs69 === errors;
if(!valid19){
break;
}
}
}
}
var valid0 = _errs67 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate446.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate446.errors = vErrors;
return errors === 0;
}
validate446.evaluated = {"props":{"branch_constraints":true,"branch_susceptance_formula":true,"constraints":true,"diagnostics":true,"dispatch":true,"flows":true,"formulation":true,"initialization":true,"injections":true,"iterations":true,"limit_rounds":true,"lmp":true,"lmp_q":true,"objective":true,"objective_kind":true,"sensitivities":true,"status":true,"va":true,"vm":true,"w":true,"wi":true,"wr":true},"dynamicProps":false,"dynamicItems":false};


function validate535(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate535.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((data.base_input === undefined) && (missing0 = "base_input")) || ((data.input === undefined) && (missing0 = "input"))) || ((data.solution === undefined) && (missing0 = "solution"))) || ((data.view === undefined) && (missing0 = "view"))) || ((data.display_solution === undefined) && (missing0 = "display_solution"))){
validate535.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.base_input !== undefined){
const _errs1 = errors;
if(typeof data.base_input !== "string"){
validate535.errors = [{instancePath:instancePath+"/base_input",schemaPath:"#/properties/base_input/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.display_solution !== undefined){
const _errs3 = errors;
if(!(validate411(data.display_solution, {instancePath:instancePath+"/display_solution",parentData:data,parentDataProperty:"display_solution",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate411.errors : vErrors.concat(validate411.errors);
errors = vErrors.length;
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.input !== undefined){
const _errs4 = errors;
if(typeof data.input !== "string"){
validate535.errors = [{instancePath:instancePath+"/input",schemaPath:"#/properties/input/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.solution !== undefined){
const _errs6 = errors;
if(typeof data.solution !== "string"){
validate535.errors = [{instancePath:instancePath+"/solution",schemaPath:"#/properties/solution/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.view !== undefined){
const _errs8 = errors;
if(!(validate446(data.view, {instancePath:instancePath+"/view",parentData:data,parentDataProperty:"view",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate446.errors : vErrors.concat(validate446.errors);
errors = vErrors.length;
}
var valid0 = _errs8 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
else {
validate535.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate535.errors = vErrors;
return errors === 0;
}
validate535.evaluated = {"props":{"base_input":true,"display_solution":true,"input":true,"solution":true,"view":true},"dynamicProps":false,"dynamicItems":false};

export const http_sensitivity = validate538;
const schema187 = {"properties":{"branch":{"format":"uint","minimum":0,"type":["integer","null"]},"bus":{"format":"uint","minimum":0,"type":["integer","null"]},"case":{"type":"string"},"operand":{"type":"string"},"parameter":{"type":"string"},"units":{"type":"string"},"values":{"items":{"$ref":"#/$defs/SensitivityValue"},"type":"array"}},"required":["case","operand","parameter","units","values"],"type":"object"};
const schema188 = {"properties":{"bus":{"format":"uint","minimum":0,"type":"integer"},"value":{"format":"double","type":"number"}},"required":["bus","value"],"type":"object"};

function validate517(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate517.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((data.bus === undefined) && (missing0 = "bus")) || ((data.value === undefined) && (missing0 = "value"))){
validate517.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.bus !== undefined){
let data0 = data.bus;
const _errs1 = errors;
if(!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))){
validate517.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: "integer"},message:"must be integer"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate517.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.value !== undefined){
let data1 = data.value;
const _errs3 = errors;
if(errors === _errs3){
if(!((typeof data1 == "number") && (isFinite(data1)))){
validate517.errors = [{instancePath:instancePath+"/value",schemaPath:"#/properties/value/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
}
}
}
else {
validate517.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate517.errors = vErrors;
return errors === 0;
}
validate517.evaluated = {"props":{"bus":true,"value":true},"dynamicProps":false,"dynamicItems":false};


function validate538(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate538.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((data.case === undefined) && (missing0 = "case")) || ((data.operand === undefined) && (missing0 = "operand"))) || ((data.parameter === undefined) && (missing0 = "parameter"))) || ((data.units === undefined) && (missing0 = "units"))) || ((data.values === undefined) && (missing0 = "values"))){
validate538.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch !== undefined){
let data0 = data.branch;
const _errs1 = errors;
if((!(((typeof data0 == "number") && (!(data0 % 1) && !isNaN(data0))) && (isFinite(data0)))) && (data0 !== null)){
validate538.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/type",keyword:"type",params:{type: schema187.properties.branch.type},message:"must be integer,null"}];
return false;
}
if(errors === _errs1){
if((typeof data0 == "number") && (isFinite(data0))){
if(data0 < 0 || isNaN(data0)){
validate538.errors = [{instancePath:instancePath+"/branch",schemaPath:"#/properties/branch/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.bus !== undefined){
let data1 = data.bus;
const _errs3 = errors;
if((!(((typeof data1 == "number") && (!(data1 % 1) && !isNaN(data1))) && (isFinite(data1)))) && (data1 !== null)){
validate538.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/type",keyword:"type",params:{type: schema187.properties.bus.type},message:"must be integer,null"}];
return false;
}
if(errors === _errs3){
if((typeof data1 == "number") && (isFinite(data1))){
if(data1 < 0 || isNaN(data1)){
validate538.errors = [{instancePath:instancePath+"/bus",schemaPath:"#/properties/bus/minimum",keyword:"minimum",params:{comparison: ">=", limit: 0},message:"must be >= 0"}];
return false;
}
}
}
var valid0 = _errs3 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.case !== undefined){
const _errs5 = errors;
if(typeof data.case !== "string"){
validate538.errors = [{instancePath:instancePath+"/case",schemaPath:"#/properties/case/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs5 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.operand !== undefined){
const _errs7 = errors;
if(typeof data.operand !== "string"){
validate538.errors = [{instancePath:instancePath+"/operand",schemaPath:"#/properties/operand/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.parameter !== undefined){
const _errs9 = errors;
if(typeof data.parameter !== "string"){
validate538.errors = [{instancePath:instancePath+"/parameter",schemaPath:"#/properties/parameter/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.units !== undefined){
const _errs11 = errors;
if(typeof data.units !== "string"){
validate538.errors = [{instancePath:instancePath+"/units",schemaPath:"#/properties/units/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs11 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.values !== undefined){
let data6 = data.values;
const _errs13 = errors;
if(errors === _errs13){
if(Array.isArray(data6)){
var valid1 = true;
const len0 = data6.length;
for(let i0=0; i0<len0; i0++){
const _errs15 = errors;
if(!(validate517(data6[i0], {instancePath:instancePath+"/values/" + i0,parentData:data6,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate517.errors : vErrors.concat(validate517.errors);
errors = vErrors.length;
}
var valid1 = _errs15 === errors;
if(!valid1){
break;
}
}
}
else {
validate538.errors = [{instancePath:instancePath+"/values",schemaPath:"#/properties/values/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
else {
validate538.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate538.errors = vErrors;
return errors === 0;
}
validate538.evaluated = {"props":{"branch":true,"bus":true,"case":true,"operand":true,"parameter":true,"units":true,"values":true},"dynamicProps":false,"dynamicItems":false};

export const http_solution = validate540;

function validate540(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate540.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if(((((((data.objective === undefined) && (missing0 = "objective")) || ((data.prices === undefined) && (missing0 = "prices"))) || ((data.va === undefined) && (missing0 = "va"))) || ((data.w === undefined) && (missing0 = "w"))) || ((data.flows === undefined) && (missing0 = "flows"))) || ((data.dispatch === undefined) && (missing0 = "dispatch"))){
validate540.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch_constraints !== undefined){
let data0 = data.branch_constraints;
const _errs1 = errors;
if((!(Array.isArray(data0))) && (data0 !== null)){
validate540.errors = [{instancePath:instancePath+"/branch_constraints",schemaPath:"#/properties/branch_constraints/type",keyword:"type",params:{type: schema145.properties.branch_constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate412(data0[i0], {instancePath:instancePath+"/branch_constraints/" + i0,parentData:data0,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate412.errors : vErrors.concat(validate412.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.constraints !== undefined){
let data2 = data.constraints;
const _errs4 = errors;
if((!(Array.isArray(data2))) && (data2 !== null)){
validate540.errors = [{instancePath:instancePath+"/constraints",schemaPath:"#/properties/constraints/type",keyword:"type",params:{type: schema145.properties.constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs4){
if(Array.isArray(data2)){
var valid2 = true;
const len1 = data2.length;
for(let i1=0; i1<len1; i1++){
const _errs6 = errors;
if(!(validate414(data2[i1], {instancePath:instancePath+"/constraints/" + i1,parentData:data2,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate414.errors : vErrors.concat(validate414.errors);
errors = vErrors.length;
}
var valid2 = _errs6 === errors;
if(!valid2){
break;
}
}
}
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.dispatch !== undefined){
let data4 = data.dispatch;
const _errs7 = errors;
if(errors === _errs7){
if(Array.isArray(data4)){
var valid3 = true;
const len2 = data4.length;
for(let i2=0; i2<len2; i2++){
const _errs9 = errors;
if(!(validate416(data4[i2], {instancePath:instancePath+"/dispatch/" + i2,parentData:data4,parentDataProperty:i2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate416.errors : vErrors.concat(validate416.errors);
errors = vErrors.length;
}
var valid3 = _errs9 === errors;
if(!valid3){
break;
}
}
}
else {
validate540.errors = [{instancePath:instancePath+"/dispatch",schemaPath:"#/properties/dispatch/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs7 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.flows !== undefined){
let data6 = data.flows;
const _errs10 = errors;
if(errors === _errs10){
if(Array.isArray(data6)){
var valid4 = true;
const len3 = data6.length;
for(let i3=0; i3<len3; i3++){
const _errs12 = errors;
if(!(validate418(data6[i3], {instancePath:instancePath+"/flows/" + i3,parentData:data6,parentDataProperty:i3,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate418.errors : vErrors.concat(validate418.errors);
errors = vErrors.length;
}
var valid4 = _errs12 === errors;
if(!valid4){
break;
}
}
}
else {
validate540.errors = [{instancePath:instancePath+"/flows",schemaPath:"#/properties/flows/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs10 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.formulation !== undefined){
const _errs13 = errors;
if(!(validate428(data.formulation, {instancePath:instancePath+"/formulation",parentData:data,parentDataProperty:"formulation",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate428.errors : vErrors.concat(validate428.errors);
errors = vErrors.length;
}
var valid0 = _errs13 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.initialization !== undefined){
let data9 = data.initialization;
const _errs14 = errors;
const _errs15 = errors;
let valid5 = false;
const _errs16 = errors;
if(!(validate430(data9, {instancePath:instancePath+"/initialization",parentData:data,parentDataProperty:"initialization",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate430.errors : vErrors.concat(validate430.errors);
errors = vErrors.length;
}
var _valid0 = _errs16 === errors;
valid5 = valid5 || _valid0;
if(_valid0){
var props0 = {};
props0.attempts = true;
props0.requested = true;
props0.restart = true;
props0.used = true;
}
const _errs17 = errors;
if(data9 !== null){
const err0 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs17 === errors;
valid5 = valid5 || _valid0;
if(!valid5){
const err1 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate540.errors = vErrors;
return false;
}
else {
errors = _errs15;
if(vErrors !== null){
if(_errs15){
vErrors.length = _errs15;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs14 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
let data10 = data.objective;
const _errs19 = errors;
if(errors === _errs19){
if(!((typeof data10 == "number") && (isFinite(data10)))){
validate540.errors = [{instancePath:instancePath+"/objective",schemaPath:"#/properties/objective/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs19 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective_kind !== undefined){
let data11 = data.objective_kind;
const _errs21 = errors;
const _errs22 = errors;
let valid6 = false;
const _errs23 = errors;
if(!(validate436(data11, {instancePath:instancePath+"/objective_kind",parentData:data,parentDataProperty:"objective_kind",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate436.errors : vErrors.concat(validate436.errors);
errors = vErrors.length;
}
var _valid1 = _errs23 === errors;
valid6 = valid6 || _valid1;
const _errs24 = errors;
if(data11 !== null){
const err2 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs24 === errors;
valid6 = valid6 || _valid1;
if(!valid6){
const err3 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate540.errors = vErrors;
return false;
}
else {
errors = _errs22;
if(vErrors !== null){
if(_errs22){
vErrors.length = _errs22;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.prices !== undefined){
let data12 = data.prices;
const _errs26 = errors;
if(errors === _errs26){
if(Array.isArray(data12)){
var valid7 = true;
const len4 = data12.length;
for(let i4=0; i4<len4; i4++){
const _errs28 = errors;
if(!(validate438(data12[i4], {instancePath:instancePath+"/prices/" + i4,parentData:data12,parentDataProperty:i4,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate438.errors : vErrors.concat(validate438.errors);
errors = vErrors.length;
}
var valid7 = _errs28 === errors;
if(!valid7){
break;
}
}
}
else {
validate540.errors = [{instancePath:instancePath+"/prices",schemaPath:"#/properties/prices/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs26 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.status !== undefined){
const _errs29 = errors;
if(!(validate440(data.status, {instancePath:instancePath+"/status",parentData:data,parentDataProperty:"status",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate440.errors : vErrors.concat(validate440.errors);
errors = vErrors.length;
}
var valid0 = _errs29 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.va !== undefined){
let data15 = data.va;
const _errs30 = errors;
if(errors === _errs30){
if(Array.isArray(data15)){
var valid8 = true;
const len5 = data15.length;
for(let i5=0; i5<len5; i5++){
const _errs32 = errors;
if(!(validate442(data15[i5], {instancePath:instancePath+"/va/" + i5,parentData:data15,parentDataProperty:i5,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate442.errors : vErrors.concat(validate442.errors);
errors = vErrors.length;
}
var valid8 = _errs32 === errors;
if(!valid8){
break;
}
}
}
else {
validate540.errors = [{instancePath:instancePath+"/va",schemaPath:"#/properties/va/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs30 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.w !== undefined){
let data17 = data.w;
const _errs33 = errors;
if(errors === _errs33){
if(Array.isArray(data17)){
var valid9 = true;
const len6 = data17.length;
for(let i6=0; i6<len6; i6++){
const _errs35 = errors;
if(!(validate442(data17[i6], {instancePath:instancePath+"/w/" + i6,parentData:data17,parentDataProperty:i6,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate442.errors : vErrors.concat(validate442.errors);
errors = vErrors.length;
}
var valid9 = _errs35 === errors;
if(!valid9){
break;
}
}
}
else {
validate540.errors = [{instancePath:instancePath+"/w",schemaPath:"#/properties/w/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs33 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate540.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate540.errors = vErrors;
return errors === 0;
}
validate540.evaluated = {"props":{"branch_constraints":true,"constraints":true,"dispatch":true,"flows":true,"formulation":true,"initialization":true,"objective":true,"objective_kind":true,"prices":true,"status":true,"va":true,"w":true},"dynamicProps":false,"dynamicItems":false};

export const http_stream_solution = validate552;
const schema189 = {"properties":{"branch_constraints":{"items":{"$ref":"#/$defs/BranchConstraintResult"},"type":["array","null"]},"case":{"type":"string"},"constraints":{"items":{"$ref":"#/$defs/ConstraintResult"},"type":["array","null"]},"dispatch":{"items":{"$ref":"#/$defs/DispatchValue"},"type":"array"},"flows":{"items":{"$ref":"#/$defs/FlowValue"},"type":"array"},"formulation":{"$ref":"#/$defs/Problem"},"initialization":{"anyOf":[{"$ref":"#/$defs/AcPfInitialization"},{"type":"null"}]},"iterations":{"items":{"$ref":"#/$defs/SolveIteration"},"type":"array"},"objective":{"format":"double","type":"number"},"objective_kind":{"anyOf":[{"$ref":"#/$defs/ObjectiveKind"},{"type":"null"}],"description":"`cost` when the solved OPF minimized the case's generator cost,\n`feasibility` when the case declares none (`objective` is then zero\nand `prices` is empty)."},"prices":{"items":{"$ref":"#/$defs/PriceValue"},"type":"array"},"solve_ms":{"format":"double","type":"number"},"status":{"$ref":"#/$defs/SolveStatus"},"va":{"items":{"$ref":"#/$defs/ScalarValue"},"type":"array"},"w":{"items":{"$ref":"#/$defs/ScalarValue"},"type":"array"}},"required":["case","solve_ms","objective","prices","va","w","flows","dispatch","iterations"],"type":"object"};

function validate552(data, {instancePath="", parentData, parentDataProperty, rootData=data, dynamicAnchors={}}={}){
let vErrors = null;
let errors = 0;
const evaluated0 = validate552.evaluated;
if(evaluated0.dynamicProps){
evaluated0.props = undefined;
}
if(evaluated0.dynamicItems){
evaluated0.items = undefined;
}
if(errors === 0){
if(data && typeof data == "object" && !Array.isArray(data)){
let missing0;
if((((((((((data.case === undefined) && (missing0 = "case")) || ((data.solve_ms === undefined) && (missing0 = "solve_ms"))) || ((data.objective === undefined) && (missing0 = "objective"))) || ((data.prices === undefined) && (missing0 = "prices"))) || ((data.va === undefined) && (missing0 = "va"))) || ((data.w === undefined) && (missing0 = "w"))) || ((data.flows === undefined) && (missing0 = "flows"))) || ((data.dispatch === undefined) && (missing0 = "dispatch"))) || ((data.iterations === undefined) && (missing0 = "iterations"))){
validate552.errors = [{instancePath,schemaPath:"#/required",keyword:"required",params:{missingProperty: missing0},message:"must have required property '"+missing0+"'"}];
return false;
}
else {
if(data.branch_constraints !== undefined){
let data0 = data.branch_constraints;
const _errs1 = errors;
if((!(Array.isArray(data0))) && (data0 !== null)){
validate552.errors = [{instancePath:instancePath+"/branch_constraints",schemaPath:"#/properties/branch_constraints/type",keyword:"type",params:{type: schema189.properties.branch_constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs1){
if(Array.isArray(data0)){
var valid1 = true;
const len0 = data0.length;
for(let i0=0; i0<len0; i0++){
const _errs3 = errors;
if(!(validate412(data0[i0], {instancePath:instancePath+"/branch_constraints/" + i0,parentData:data0,parentDataProperty:i0,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate412.errors : vErrors.concat(validate412.errors);
errors = vErrors.length;
}
var valid1 = _errs3 === errors;
if(!valid1){
break;
}
}
}
}
var valid0 = _errs1 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.case !== undefined){
const _errs4 = errors;
if(typeof data.case !== "string"){
validate552.errors = [{instancePath:instancePath+"/case",schemaPath:"#/properties/case/type",keyword:"type",params:{type: "string"},message:"must be string"}];
return false;
}
var valid0 = _errs4 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.constraints !== undefined){
let data3 = data.constraints;
const _errs6 = errors;
if((!(Array.isArray(data3))) && (data3 !== null)){
validate552.errors = [{instancePath:instancePath+"/constraints",schemaPath:"#/properties/constraints/type",keyword:"type",params:{type: schema189.properties.constraints.type},message:"must be array,null"}];
return false;
}
if(errors === _errs6){
if(Array.isArray(data3)){
var valid2 = true;
const len1 = data3.length;
for(let i1=0; i1<len1; i1++){
const _errs8 = errors;
if(!(validate414(data3[i1], {instancePath:instancePath+"/constraints/" + i1,parentData:data3,parentDataProperty:i1,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate414.errors : vErrors.concat(validate414.errors);
errors = vErrors.length;
}
var valid2 = _errs8 === errors;
if(!valid2){
break;
}
}
}
}
var valid0 = _errs6 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.dispatch !== undefined){
let data5 = data.dispatch;
const _errs9 = errors;
if(errors === _errs9){
if(Array.isArray(data5)){
var valid3 = true;
const len2 = data5.length;
for(let i2=0; i2<len2; i2++){
const _errs11 = errors;
if(!(validate416(data5[i2], {instancePath:instancePath+"/dispatch/" + i2,parentData:data5,parentDataProperty:i2,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate416.errors : vErrors.concat(validate416.errors);
errors = vErrors.length;
}
var valid3 = _errs11 === errors;
if(!valid3){
break;
}
}
}
else {
validate552.errors = [{instancePath:instancePath+"/dispatch",schemaPath:"#/properties/dispatch/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs9 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.flows !== undefined){
let data7 = data.flows;
const _errs12 = errors;
if(errors === _errs12){
if(Array.isArray(data7)){
var valid4 = true;
const len3 = data7.length;
for(let i3=0; i3<len3; i3++){
const _errs14 = errors;
if(!(validate418(data7[i3], {instancePath:instancePath+"/flows/" + i3,parentData:data7,parentDataProperty:i3,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate418.errors : vErrors.concat(validate418.errors);
errors = vErrors.length;
}
var valid4 = _errs14 === errors;
if(!valid4){
break;
}
}
}
else {
validate552.errors = [{instancePath:instancePath+"/flows",schemaPath:"#/properties/flows/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs12 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.formulation !== undefined){
const _errs15 = errors;
if(!(validate428(data.formulation, {instancePath:instancePath+"/formulation",parentData:data,parentDataProperty:"formulation",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate428.errors : vErrors.concat(validate428.errors);
errors = vErrors.length;
}
var valid0 = _errs15 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.initialization !== undefined){
let data10 = data.initialization;
const _errs16 = errors;
const _errs17 = errors;
let valid5 = false;
const _errs18 = errors;
if(!(validate430(data10, {instancePath:instancePath+"/initialization",parentData:data,parentDataProperty:"initialization",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate430.errors : vErrors.concat(validate430.errors);
errors = vErrors.length;
}
var _valid0 = _errs18 === errors;
valid5 = valid5 || _valid0;
if(_valid0){
var props0 = {};
props0.attempts = true;
props0.requested = true;
props0.restart = true;
props0.used = true;
}
const _errs19 = errors;
if(data10 !== null){
const err0 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err0];
}
else {
vErrors.push(err0);
}
errors++;
}
var _valid0 = _errs19 === errors;
valid5 = valid5 || _valid0;
if(!valid5){
const err1 = {instancePath:instancePath+"/initialization",schemaPath:"#/properties/initialization/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err1];
}
else {
vErrors.push(err1);
}
errors++;
validate552.errors = vErrors;
return false;
}
else {
errors = _errs17;
if(vErrors !== null){
if(_errs17){
vErrors.length = _errs17;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs16 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.iterations !== undefined){
let data11 = data.iterations;
const _errs21 = errors;
if(errors === _errs21){
if(Array.isArray(data11)){
var valid6 = true;
const len4 = data11.length;
for(let i4=0; i4<len4; i4++){
const _errs23 = errors;
if(!(validate463(data11[i4], {instancePath:instancePath+"/iterations/" + i4,parentData:data11,parentDataProperty:i4,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate463.errors : vErrors.concat(validate463.errors);
errors = vErrors.length;
}
var valid6 = _errs23 === errors;
if(!valid6){
break;
}
}
}
else {
validate552.errors = [{instancePath:instancePath+"/iterations",schemaPath:"#/properties/iterations/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs21 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective !== undefined){
let data13 = data.objective;
const _errs24 = errors;
if(errors === _errs24){
if(!((typeof data13 == "number") && (isFinite(data13)))){
validate552.errors = [{instancePath:instancePath+"/objective",schemaPath:"#/properties/objective/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs24 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.objective_kind !== undefined){
let data14 = data.objective_kind;
const _errs26 = errors;
const _errs27 = errors;
let valid7 = false;
const _errs28 = errors;
if(!(validate436(data14, {instancePath:instancePath+"/objective_kind",parentData:data,parentDataProperty:"objective_kind",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate436.errors : vErrors.concat(validate436.errors);
errors = vErrors.length;
}
var _valid1 = _errs28 === errors;
valid7 = valid7 || _valid1;
const _errs29 = errors;
if(data14 !== null){
const err2 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf/1/type",keyword:"type",params:{type: "null"},message:"must be null"};
if(vErrors === null){
vErrors = [err2];
}
else {
vErrors.push(err2);
}
errors++;
}
var _valid1 = _errs29 === errors;
valid7 = valid7 || _valid1;
if(!valid7){
const err3 = {instancePath:instancePath+"/objective_kind",schemaPath:"#/properties/objective_kind/anyOf",keyword:"anyOf",params:{},message:"must match a schema in anyOf"};
if(vErrors === null){
vErrors = [err3];
}
else {
vErrors.push(err3);
}
errors++;
validate552.errors = vErrors;
return false;
}
else {
errors = _errs27;
if(vErrors !== null){
if(_errs27){
vErrors.length = _errs27;
}
else {
vErrors = null;
}
}
}
var valid0 = _errs26 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.prices !== undefined){
let data15 = data.prices;
const _errs31 = errors;
if(errors === _errs31){
if(Array.isArray(data15)){
var valid8 = true;
const len5 = data15.length;
for(let i5=0; i5<len5; i5++){
const _errs33 = errors;
if(!(validate438(data15[i5], {instancePath:instancePath+"/prices/" + i5,parentData:data15,parentDataProperty:i5,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate438.errors : vErrors.concat(validate438.errors);
errors = vErrors.length;
}
var valid8 = _errs33 === errors;
if(!valid8){
break;
}
}
}
else {
validate552.errors = [{instancePath:instancePath+"/prices",schemaPath:"#/properties/prices/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs31 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.solve_ms !== undefined){
let data17 = data.solve_ms;
const _errs34 = errors;
if(errors === _errs34){
if(!((typeof data17 == "number") && (isFinite(data17)))){
validate552.errors = [{instancePath:instancePath+"/solve_ms",schemaPath:"#/properties/solve_ms/type",keyword:"type",params:{type: "number"},message:"must be number"}];
return false;
}
}
var valid0 = _errs34 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.status !== undefined){
const _errs36 = errors;
if(!(validate440(data.status, {instancePath:instancePath+"/status",parentData:data,parentDataProperty:"status",rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate440.errors : vErrors.concat(validate440.errors);
errors = vErrors.length;
}
var valid0 = _errs36 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.va !== undefined){
let data19 = data.va;
const _errs37 = errors;
if(errors === _errs37){
if(Array.isArray(data19)){
var valid9 = true;
const len6 = data19.length;
for(let i6=0; i6<len6; i6++){
const _errs39 = errors;
if(!(validate442(data19[i6], {instancePath:instancePath+"/va/" + i6,parentData:data19,parentDataProperty:i6,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate442.errors : vErrors.concat(validate442.errors);
errors = vErrors.length;
}
var valid9 = _errs39 === errors;
if(!valid9){
break;
}
}
}
else {
validate552.errors = [{instancePath:instancePath+"/va",schemaPath:"#/properties/va/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs37 === errors;
}
else {
var valid0 = true;
}
if(valid0){
if(data.w !== undefined){
let data21 = data.w;
const _errs40 = errors;
if(errors === _errs40){
if(Array.isArray(data21)){
var valid10 = true;
const len7 = data21.length;
for(let i7=0; i7<len7; i7++){
const _errs42 = errors;
if(!(validate442(data21[i7], {instancePath:instancePath+"/w/" + i7,parentData:data21,parentDataProperty:i7,rootData,dynamicAnchors}))){
vErrors = vErrors === null ? validate442.errors : vErrors.concat(validate442.errors);
errors = vErrors.length;
}
var valid10 = _errs42 === errors;
if(!valid10){
break;
}
}
}
else {
validate552.errors = [{instancePath:instancePath+"/w",schemaPath:"#/properties/w/type",keyword:"type",params:{type: "array"},message:"must be array"}];
return false;
}
}
var valid0 = _errs40 === errors;
}
else {
var valid0 = true;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
else {
validate552.errors = [{instancePath,schemaPath:"#/type",keyword:"type",params:{type: "object"},message:"must be object"}];
return false;
}
}
validate552.errors = vErrors;
return errors === 0;
}
validate552.evaluated = {"props":{"branch_constraints":true,"case":true,"constraints":true,"dispatch":true,"flows":true,"formulation":true,"initialization":true,"iterations":true,"objective":true,"objective_kind":true,"prices":true,"solve_ms":true,"status":true,"va":true,"w":true},"dynamicProps":false,"dynamicItems":false};
