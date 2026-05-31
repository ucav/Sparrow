# Skill: Local Temperature

**Trigger:** temperature, weather, local temperature, current temperature

**Description:** Get the current local temperature using the wttr.in service.

## Body
To get the local temperature, run:
```sh
curl -s "wttr.in?format=%t"
```
This returns the temperature (e.g., "+22°C"). You can also get a more detailed report with:
```sh
curl -s "wttr.in?format=3"
```
which outputs something like: "London: +22°C ☀️".

Note: This skill requires internet access and the `curl` command available in the environment.