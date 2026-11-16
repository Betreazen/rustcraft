"""Console commands for a RustCraft load scene on tidewill (RustCraft's area, x 0..1000).

A stone platform at y 150 (above any terrain), ROWS lines of magistral, each with MACHINES machines
along it: every machine with a crystal on top (it powers what it touches) and a stack of input.
Usage: py -3 stress_scene.py build|clear [rows] [machines] > commands.txt"""
import sys

X0, Y, Z0 = 600, 150, 600
KINDS = [('rustcraft:furnace', 'minecraft:raw_iron'), ('rustcraft:crusher', 'minecraft:raw_copper'),
         ('rustcraft:furnace', 'minecraft:raw_gold'), ('rustcraft:crusher', 'minecraft:raw_iron')]


def build(rows, machines):
    out = []
    w, d = machines * 2 + 6, rows * 4 + 6
    out.append('forceload add %d %d %d %d' % (X0 - 4, Z0 - 4, X0 + w, Z0 + d))
    out.append('fill %d %d %d %d %d %d minecraft:smooth_stone' % (X0 - 2, Y - 1, Z0 - 2, X0 + w, Y - 1, Z0 + d))
    for r in range(rows):
        z = Z0 + r * 4
        # The magistral: a node at each end, housings between, in front of the machines.
        out.append('setblock %d %d %d rustcraft:node' % (X0 - 1, Y, z + 1))
        out.append('fill %d %d %d %d %d %d rustcraft:housing' % (X0, Y, z + 1, X0 + machines * 2, Y, z + 1))
        out.append('setblock %d %d %d rustcraft:node' % (X0 + machines * 2 + 1, Y, z + 1))
        for i in range(machines):
            x = X0 + i * 2
            block, item = KINDS[(r + i) % len(KINDS)]
            out.append('setblock %d %d %d %s' % (x, Y, z, block))
            out.append('setblock %d %d %d rustcraft:crystal' % (x, Y + 1, z))
            out.append('item replace block %d %d %d container.0 with %s 64' % (x, Y, z, item))
    return out


def clear(rows, machines):
    w, d = machines * 2 + 6, rows * 4 + 6
    return ['fill %d %d %d %d %d %d minecraft:air' % (X0 - 2, Y, Z0 - 2, X0 + w, Y + 3, Z0 + d),
            'fill %d %d %d %d %d %d minecraft:air' % (X0 - 2, Y - 1, Z0 - 2, X0 + w, Y - 1, Z0 + d),
            'forceload remove %d %d %d %d' % (X0 - 4, Z0 - 4, X0 + w, Z0 + d)]


if __name__ == '__main__':
    mode = sys.argv[1]
    rows = int(sys.argv[2]) if len(sys.argv) > 2 else 8
    machines = int(sys.argv[3]) if len(sys.argv) > 3 else 16
    print('\n'.join((build if mode == 'build' else clear)(rows, machines)))
