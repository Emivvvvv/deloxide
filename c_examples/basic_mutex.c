#include "deloxide.h"

int main(void) {
    int initialized = deloxide_init(NULL, NULL);
    if (initialized != 0 && initialized != 1) {
        return 1;
    }

    void *mutex = deloxide_create_mutex();
    if (mutex == NULL || deloxide_lock_mutex(mutex) != 0) {
        return 2;
    }
    if (deloxide_unlock_mutex(mutex) != 0) {
        return 3;
    }
    deloxide_destroy_mutex(mutex);
    return 0;
}
