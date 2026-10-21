"""Normalize explicitly declared CPU instruction-to-guest-tick ratios."""


def normalize_instructions_per_tick(value):
    """Return (numerator, denominator) for a declared CPU-speed ratio.

    The camera's true CPU clock is unknown; this is a declared host input
    (instructions executed per virtual timer tick). Accepts a positive int,
    ``"n/d"`` text, a Fraction or an ``(n, d)`` pair. Default 1 reproduces
    the historical one-tick-per-instruction behavior.
    """
    from fractions import Fraction
    if isinstance(value, bool):
        raise ValueError("instructions_per_tick must not be boolean")
    if isinstance(value, tuple):
        value = Fraction(*value)
    elif isinstance(value, str):
        value = Fraction(value)
    elif isinstance(value, int):
        value = Fraction(value)
    elif not isinstance(value, Fraction):
        raise ValueError("instructions_per_tick must be an int, 'n/d' text, Fraction or pair")
    if value < 1 or value.numerator > 1 << 32 or value.denominator > 1 << 32:
        raise ValueError("instructions_per_tick must be at least 1 and a bounded ratio")
    return value.numerator, value.denominator


def ratio_option(ratio):
    """Factory-option spelling of a normalized ratio (int, or 'n/d')."""
    return ratio[0] if ratio[1] == 1 else f"{ratio[0]}/{ratio[1]}"
