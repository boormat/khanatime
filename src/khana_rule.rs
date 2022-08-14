pub const RULES_MARKDOWN: &str = r####"
# Penalties

## Slowest time plus five (5) seconds
1. **WD** Wrong direction

1. **DNF** Any other action that can be deemed as incorrectly
completing that course (such as reversing after exceed the limits of a garage)

1. **DNF** Failure to complete a test

1. **ROO** Running out of order (without the prior approval of the Clerk of the
Course)

1. **FTS** Failing to stop completely within a mid-course garage

1. **FTS** Failing to stop completely at the finish of a test

1. **FTS** Finish a test with the car stopped but completely outside the garage

## Plus five (5) seconds
1. **FLAG** Finishing a test with part of the car outside the garage boundaries.

## Plus five (5) seconds per flag/marker
1. **FLAG** Striking a course flag/marker (including garage boundary 
flag/marker)

## Slowest time plus ten (10) seconds
1. **NOSHO** Failure to attempt a test


## Plus five seconds
1. Plus five (5) seconds which means that five seconds must be added to the time
recorded by that driver on that test for each infringement.

_This is for a completed course. It is not applied where slowest time plus
penalties have been applied._

## Slowest time plus

These penaties become the time recorded for the penalised driver. Slowest time
is calculated as the minimum of:

1. The slowest time recorded by a driver who completed that test correctly and
**No** penalties

1. double the fastest time.
"####;
