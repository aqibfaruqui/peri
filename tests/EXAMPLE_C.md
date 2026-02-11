## Example generated C header
```c
// Dummy structs for each state
typedef struct { int _unused; } Timer_Disabled;
typedef struct { int _unused; } Timer_Enabled;

// Initial state constructor
Timer_Disabled* get_timer(void);

// fn enable_timer() :: Timer<Disabled> -> Timer<Enabled> { ... }
Timer_Enabled* enable_timer(Timer_Disabled* h);

// fn stop_timer() :: Timer<Enabled> -> Timer<Disabled> { ... }
Timer_Disabled* stop_timer(Timer_Enabled* h);
```

## How to use as a C developer
```c
void main() {
    // Start with Disabled
    Timer_Disabled* t_off = get_timer();
    
    // Correct usage: chain the pointers
    Timer_Enabled* t_on = enable_timer(t_off);
    
    // ... use t_on ...
    
    Timer_Disabled* t_off2 = stop_timer(t_on);
}
```

## What happens if used incorrectly
```c
void main() {
    Timer_Disabled* t_off = get_timer();
    
    // ERROR: stop_timer expects Timer_Enabled*, passing Timer_Disabled*
    stop_timer(t_off); 
}
```