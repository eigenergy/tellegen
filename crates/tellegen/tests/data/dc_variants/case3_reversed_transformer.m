% Synthetic regression fixture, MIT, Tellegen contributors.
function mpc = case3_reversed_transformer
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0 0 0 0 1 1 0 230 1 1.1 0.9;
 2 1 90 20 0 0 1 1 0 230 1 1.1 0.9;
 3 2 0 0 0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
 1 30 0 300 -300 1 100 1 250 0 0 0 0 0 0 0 0 0 0 0 0;
 3 60 0 300 -300 1 100 1 270 0 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.03 0.1 0 32 32 32 1.1 2 1 -25 40;
 1 3 0.01 0.13 0 250 250 250 0 0 1 -60 60;
 2 3 0.02 0.08 0 250 250 250 0 0 1 -60 60;
 2 1 0.04 0.16 0 250 250 250 1.2 -3 1 -35 45;
];
mpc.gencost = [
 2 0 0 3 0.02 5 3;
 2 0 0 3 0.04 10 7;
];
