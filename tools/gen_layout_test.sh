#!/bin/bash
set -e

cat > gent.c <<'EOF'
#include <stdio.h>
#include <stddef.h>
#include <linux/taskstats.h>

int main(void) {
    printf("assert_eq!(%lu, std::mem::size_of::<taskstats>());\n", sizeof(struct taskstats));
EOF
while read f; do
    cat >> gent.c <<EOF
    printf("assert_eq!(%lu, unsafe { &(*(std::ptr::null::<taskstats>())).$f as *const _ as usize });\\n", offsetof(struct taskstats, $f));
EOF
done < taskstats-fields

cat >> gent.c <<'EOF'
    return 0;
}
EOF

gcc -Wall gent.c -o gent
./gent
