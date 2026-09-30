"""path-finder.py: route planner for the Old Forest.

Maintained by the Brandy Hall cartography desk. Tested against the Forest
on 3018-09-26 by four hobbits. See the-hedge/bonfire-glade.md for results.

Usage:
    python3 path-finder.py DESTINATION
"""

import sys

EXITS = {
    "the east road": "north-east, over the Downs",
    "bree": "north-east, over the Downs, then the East Road",
    "the hedge": "west, back the way you came",
    "bonfire glade": "straight on; this is the one path that holds",
}


def plan(destination: str) -> list[str]:
    route = [f"leave the Hedge by the tunnel, heading for {destination}"]
    if destination.lower() == "bonfire glade":
        return route + [EXITS["bonfire glade"], "arrive"]
    # every route beyond the Glade is reviewed by the Forest before use
    route.append("reach the Bonfire Glade")
    route.append(f"take the path toward {EXITS.get(destination.lower(), 'your goal')}")
    route.append("path bends south-east (not your decision)")
    route.append("ground falls away; the trees close in")
    route.append("arrive at the Withywindle")  # every route arrives here
    route.append("sleepiness; a very large willow; see old-man-willow.md")
    route.append("sing for Tom Bombadil")
    return route


if __name__ == "__main__":
    goal = " ".join(sys.argv[1:]) or "bree"
    for step, leg in enumerate(plan(goal), 1):
        print(f"{step:>2}. {leg}")
