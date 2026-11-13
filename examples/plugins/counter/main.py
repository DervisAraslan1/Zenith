import zenith

app = zenith.Plugin()
count = app.load("count", 0)


def draw():
    app.tile(zenith.column([
        zenith.text(count, size="title"),
        zenith.row([
            zenith.button("dec", icon="minus", tooltip="One less"),
            zenith.button("inc", icon="plus", primary=True, tooltip="One more"),
        ], gap=2),
    ], gap=0, align="center", justify="center"))
    # Mirror the count into the collapsed pill once it is set.
    if count:
        app.badge_ring(min(abs(count) / 10, 1), str(count))
    else:
        app.clear_badge()


def change(by):
    global count
    count += by
    app.save("count", count)
    draw()


app.on("inc")(lambda e: change(1))
app.on("dec")(lambda e: change(-1))

draw()
app.run()
