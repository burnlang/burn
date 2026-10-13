long burn_compare_ints(long a, long b);
double burn_twice(double x, long n);
long burn_count_up(long from);

void *c_compare_ints_address(void) {
    return (void *)burn_compare_ints;
}

double c_twice_via_burn(double x) {
    return burn_twice(x, 3) + 0.5;
}

long c_calls_burn(long x) {
    return burn_count_up(x) * 10;
}
