#!/bin/python3
from collections import defaultdict
import os
import operator

NUMBER_RANKED = 10

FILE_STORE="/var/opt/keystats/previous"
CURRENT_FILE="/var/opt/keystats/keys"

COUNT_OFFSET = 0
VAL_OFFSET = 1
CTRL_OFFSET = 2
SHIFT_OFFSET = 3
ALT_OFFSET = 4
META_OFFSET = 5
MODIFIERS_VAL = ("Shift", "Alt", "Ctrl", "Meta")

individual_keys = defaultdict(lambda: 0)
combinations = defaultdict(lambda: 0)
modifiers = defaultdict(lambda: 0)
all_goes = defaultdict(lambda: 0)
total_modifiers = 0
total_regular = 0
total_combinations = 0

def handle_line(line):
    global total_modifiers
    global total_regular
    global total_combinations
    
    data = line.split()
    for i in range(2, 6):
        data[i] = False if data[i] == "false" else True
    val = data[VAL_OFFSET]
    cnt = int(data[COUNT_OFFSET])

    all_goes[val] += cnt
    # key IS a modifier
    if val in MODIFIERS_VAL:
        total_modifiers += cnt
        modifiers[val] += cnt
    else:
        comb_key = tuple(data[1:])
        # at least one modifier is on
        if comb_key[1] or comb_key[2] or comb_key[3] or comb_key[4]:
            combinations[comb_key] += cnt
            total_combinations += cnt
        # no modifier
        else:
            total_regular += cnt
            individual_keys[val] += cnt

def handle_file(file_name):
    with open(file_name) as f:
        for line in f:
            handle_line(line)

def print_stats():
    sorted_individual_keys = sorted(individual_keys.items(), key=operator.itemgetter(1))[::-1]
    sorted_combinations = sorted(combinations.items(), key=operator.itemgetter(1))[::-1]
    sorted_modifiers = sorted(modifiers.items(), key=operator.itemgetter(1))[::-1]
    sorted_all_goes = sorted(all_goes.items(), key=operator.itemgetter(1))[::-1]

    total = total_regular + total_modifiers + total_combinations
    print("TOTAL KEYS PRESSED: %d\n______________________" % total)

    i = 0
    print("\nMODIFIERS: %2.2f%%\n" % (100*total_modifiers/total))
    for (key, val) in sorted_modifiers:
        i += 1
        print("%d. %s (%2.2f%%)" % (i, key, 100*val/total_modifiers))

    i = 0
    print("\nCOMBINATIONS: %2.2f%%\n" % (100*total_combinations/total))
    for (key, val) in sorted_combinations:
        combination = key[0]
        if key[1]:
            combination = "ctrl + " + combination
        if key[2]:
            combination = "shift + " + combination
        if key[3]:
            combination = "alt + " + combination
        if key[4]:
            combination = "meta + " + combination

        i += 1
        print("%d. %s (%2.2f%%)" % (i, combination, 100*val/total_combinations))
        if i >= NUMBER_RANKED:
            break

    i = 0
    print("\nNO_MODIFIER REGULAR KEYS: %2.2f%%\n" % (100*total_regular/total))
    for (key, val) in sorted_individual_keys:
        i += 1
        print("%d. %s (%2.2f%%)" % (i, key, 100*val/total_regular))
        if i >= NUMBER_RANKED:
            break

    i = 0
    print("\nALL KEYS\n")
    for (key, val) in sorted_all_goes:
        i += 1
        print("%d. %s (%2.2f%%)" % (i, key, 100*val/total))
        if i >= NUMBER_RANKED:
            break


if __name__ == "__main__":
    for file_ in os.scandir(FILE_STORE):
        handle_file(file_.path)
    handle_file(CURRENT_FILE)
    print_stats()
