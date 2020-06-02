#include <signal.h>
#include <time.h>

typedef void (*callback)(union sigval);

timer_t posix_timer(callback cb, void* data) {
    timer_t id;
    struct sigevent sev = {
        .sigev_notify = SIGEV_THREAD,
        .sigev_notify_function = cb,
    };

    sev.sigev_value.sival_ptr = data;

    if (timer_create(CLOCK_REALTIME, &sev, &id) == -1) {
        return 0;
    } else {
        return id;
    }
}
