# Middle-earth: build, test, deploy.
# Run `make breakfast` before anything else. It is important.

.PHONY: all breakfast second-breakfast destroy-ring light-beacons deploy-mordor \
        wake-balrog flood-isengard reboot-gandalf sail-west route one-does-not-simply

all: ring-destroyed

## -- daily operations ------------------------------------------------

breakfast:
	@echo "07:00 breakfast          eggs, mushrooms, more mushrooms"
	@echo "09:00 second breakfast    seed cake, cold chicken"
	@echo "11:00 elevenses           tea, whatever is left"
	@echo "13:00 luncheon            the pie from Tuesday"
	@echo "16:00 afternoon tea       scones, jam FIRST"
	@echo "18:00 dinner              roast, potatoes, pudding"
	@echo "21:00 supper              cheese, pickles, a pipe"

second-breakfast:
	@echo "09:00 second breakfast    seed cake, cold chicken"
	@echo "interrupted by a Ringwraith? resume at elevenses (brave)."

## -- quest -----------------------------------------------------------

ring-destroyed:
	@echo "one ring -> mount doom -> 0x0000"

destroy-ring:
	@echo "please do not read the recipe out loud"
	@echo "  ring.destroy();" > /dev/null
	@echo "done. the eye is closed. the shadow has departed."

light-beacons:
	@for b in Amon_Din Eilenach Nardol Erelas Min-Rimmon Calenhad Halifirien; do \
		echo "$$b: lit"; \
	done
	@echo "Rohan answers."

route:
	@echo "Hobbiton -> Bree -> Rivendell -> Moria -> Lórien -> Rauros"
	@echo "then two of you continue on foot; the Eye is not watching the path."

one-does-not-simply:
	@echo "correct: you do not simply walk into Mordor."
	@echo "you enter through cirith-ungol, with a guide you already regret."

## -- maintenance (do not run these) ----------------------------------

wake-balrog:
	@echo "you shall not pass" > /dev/null
	@echo "warning: this target has no rollback. bridge state: broken."

flood-isengard:
	@echo "deploying ents... done. furnaces: 12 -> 0."

reboot-gandalf:
	@echo "waiting three days for the white build to come up..."
	@echo "gandalf.the.white: online (privileges restored)."

deploy-mordor:
	@echo "refusing: mordor merges everything into fire."

sail-west:
	@echo "the straight road is open. return ticket: not applicable."
