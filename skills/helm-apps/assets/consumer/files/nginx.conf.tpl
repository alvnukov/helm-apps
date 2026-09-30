events {}
http {
  server {
    listen {{ include "fl.value" (list $ . $.Values.global.vars.port) }};
    location / {
      return 200 "{{ $.CurrentApp.name }} / {{ $.CurrentContainer.name }}\n";
    }
  }
}
