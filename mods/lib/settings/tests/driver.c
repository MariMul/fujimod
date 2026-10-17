/* Host qualification of the upstream generated types and parser.
 * No camera access; candidate publication is explicitly caller-owned. */
#include <stdio.h>
#include <string.h>
#include "section.h"

static settings_section_t live;
static char input[4096];

static void hex(const char *s)
{
    while (*s) printf("%02x", (unsigned char)*s++);
}

int main(void)
{
    settings_section_t candidate;
    size_t length;
    unsigned int i;
    _Static_assert(sizeof(candidate.author) == 33, "Strings must be inline and bounded");
    _Static_assert(sizeof(candidate.points.items) == 4 * sizeof(uint32_t), "Arrays must be inline");
    memset(&live, 0, sizeof live);
    live.hold_ms = 999;
    length = fread(input, 1, sizeof(input)-1, stdin);
    input[length] = 0;
    memset(&candidate, 0, sizeof candidate);
    if (json_parse_settings_section_with_len(input, length, &candidate)) {
        printf("error\t%u\n", live.hold_ms);
        return 0;
    }
    live = candidate;
    /* Input must not own any live strings: overwrite it before inspection. */
    memset(input, 'x', length);
    printf("ok\t%d\t%u\t%d\t%u\t", live.enabled, live.hold_ms,
           live.theme, live.controls.repeat_ms);
    hex(live.author);
    printf("\t%llu", (unsigned long long)live.points.n);
    for (i = 0; i < live.points.n; ++i) printf("\t%u", live.points.items[i]);
    putchar('\n');
    return 0;
}
