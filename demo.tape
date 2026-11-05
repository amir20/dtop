# Recording setup, so the GIF shows only the demo stack and no local config:
#   docker build -t dtop-demo .
#   docker compose -f scripts/demo-compose.yaml up -d
#   vhs demo.tape
#   docker compose -f scripts/demo-compose.yaml down -t 1
#   docker ps -q --filter ancestor=dtop-demo | xargs docker rm -f  # vhs leaves dtop running

Set Theme "Ayu Mirage"
Output demo.gif

Set TypingSpeed 100ms
Set Shell "bash"
Set Width 2100
Set Height 800

Hide
Type `alias dtop='docker run --rm -it --hostname demo --network host -v /var/run/docker.sock:/var/run/docker.sock dtop-demo --filter label=dtop.demo'; clear`
Enter
Sleep 500ms
Show

Type "dtop"
Sleep 700ms
Enter
Sleep 7s

Type "/api"
Sleep 500ms
Enter
Sleep 2s

Right
Sleep 4s
Left
Sleep 1s

Type "/"
Escape
Sleep 2s

Space
Sleep 400ms
Space
Sleep 400ms
Space
Sleep 800ms
Enter
Sleep 2.5s
Escape
Sleep 500ms
Escape
Sleep 2s
