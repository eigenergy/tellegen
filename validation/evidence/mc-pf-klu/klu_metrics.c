/* Test harness for SuiteSparse KLU 5.6's published C API. */
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "klu.h"

static double seconds(void) {
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (double)now.tv_sec + (double)now.tv_nsec / 1e9;
}

/* metrics: factor seconds, warm solve seconds, retained bytes, peak bytes,
   factor nonzeros. Caller owns all input arrays and output. */
int compare_klu(int n, int *columns, int *rows, double *values, double *rhs,
                int ordering, double *out, double *metrics) {
    klu_common common;
    klu_defaults(&common);
    common.ordering = ordering;
    common.halt_if_singular = 1;
    double start = seconds();
    klu_symbolic *symbolic = klu_analyze(n, columns, rows, &common);
    if (!symbolic) return common.status;
    klu_numeric *numeric = klu_z_factor(columns, rows, values, symbolic, &common);
    if (!numeric) { klu_free_symbolic(&symbolic, &common); return common.status; }
    metrics[0] = seconds() - start;
    metrics[2] = (double)common.memusage;
    metrics[3] = (double)common.mempeak;
    metrics[4] = (double)(numeric->lnz + numeric->unz + numeric->nzoff);
    memcpy(out, rhs, (size_t)(2*n)*sizeof(double));
    int success = klu_z_solve(symbolic, numeric, n, 1, out, &common);
    start = seconds();
    for (int i = 0; i < 20 && success; ++i) {
        memcpy(out, rhs, (size_t)(2*n)*sizeof(double));
        success = klu_z_solve(symbolic, numeric, n, 1, out, &common);
    }
    metrics[1] = (seconds() - start)/20.0;
    klu_z_condest(columns, values, symbolic, numeric, &common);
    metrics[5] = common.condest;
    klu_z_rcond(symbolic, numeric, &common);
    metrics[6] = common.rcond;
    klu_z_free_numeric(&numeric, &common);
    klu_free_symbolic(&symbolic, &common);
    return success ? 0 : -1;
}
