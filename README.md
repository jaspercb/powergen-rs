# powergen-rs

## What's this, then?

Say you're writing a game and you want to procedurally generate projectiles with different effects. One tempting way to do this is by splitting up these projectiles into two parts;

* the part that governs the projectile's motion - speed, acceleration, homing, etc.

* the part that applies an effect to the unlucky victim

```
+------------+   Entity    +----------+
| projectile | ----------> | callback |
+------------+             +----------+
```

This project is me trying to generalize this to larger graphs, containing nodes that might have more than one input and more than one output. I want to procedurally fit together pieces like

```
 position -> +------------+
             | projectile | -> entity
direction -> +------------+

            +-------------+
entity ---> | location_of | -> position
            +-------------+

position -> +-----------+
            | explosion | -> entity
float ----> +-----------+

            +-----------+
            |   const   | -> float
            +-----------+
```

into larger networks of callbacks that (for example) fire a projectile that hits someone, explodes with some fixed (or perhaps variable!) radius, and does something to all entities caught in the explosion.
