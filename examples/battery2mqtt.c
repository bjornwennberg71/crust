/*
 * battery2mqtt.c — simulates a multi-module battery pack and publishes
 * voltage / SoC readings to MQTT topics.
 *
 * Build:
 *   gcc -O2 -o battery2mqtt battery2mqtt.c -lmosquitto -lm
 *
 * Topics published:
 *   <prefix>/<id>/voltage              aggregate voltage (average of modules)
 *   <prefix>/<id>/soc                  aggregate SoC    (average of modules)
 *   <prefix>/<id>/module/<m>/voltage
 *   <prefix>/<id>/module/<m>/soc
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <math.h>
#include <mosquitto.h>

#define MAX_BATTERIES   8
#define MAX_MODULES    10
#define MAX_STR        256

/* ------------------------------------------------------------------ types */

typedef struct
{
    double voltage;
    double soc;
} Module;

typedef struct
{
    int    id;
    int    num_modules;
    Module modules[MAX_MODULES];
    double voltage;    /* aggregate */
    double soc;        /* aggregate */
} Battery;

typedef struct
{
    char   mqtt_host[MAX_STR];
    int    mqtt_port;
    char   topic_prefix[MAX_STR];
    int    num_batteries;
    int    modules_per_battery;
    int    interval_ms;
} Config;

/* --------------------------------------------------------------- defaults */

static void default_config(Config *c)
{
    strncpy(c->mqtt_host,     "localhost", MAX_STR);
    strncpy(c->topic_prefix,  "battery",   MAX_STR);
    c->mqtt_port           = 1883;
    c->num_batteries       = 1;
    c->modules_per_battery = 4;
    c->interval_ms         = 1000;
}

/* ---------------------------------------------------------- config parser */

static void trim_right(char *s)
{
    char *end = s + strlen(s) - 1;
    while (end >= s && (*end == ' ' || *end == '\t' || *end == '\r'))
        *end-- = '\0';
}

static int load_config(Config *c, const char *path)
{
    FILE *f = fopen(path, "r");
    if (!f) return -1;

    char line[512];
    while (fgets(line, sizeof(line), f))
    {
        line[strcspn(line, "\n")] = 0;
        if (line[0] == '#' || line[0] == '\0') continue;

        char key[256], val[256];
        if (sscanf(line, " %255[^=]= %255[^\n]", key, val) != 2) continue;
        trim_right(key);

        if      (!strcmp(key, "mqtt_host"))           strncpy(c->mqtt_host,    val, MAX_STR);
        else if (!strcmp(key, "mqtt_port"))           c->mqtt_port           = atoi(val);
        else if (!strcmp(key, "topic_prefix"))        strncpy(c->topic_prefix, val, MAX_STR);
        else if (!strcmp(key, "num_batteries"))       c->num_batteries       = atoi(val);
        else if (!strcmp(key, "modules_per_battery")) c->modules_per_battery = atoi(val);
        else if (!strcmp(key, "interval_ms"))         c->interval_ms         = atoi(val);
    }
    fclose(f);
    return 0;
}

/* ----------------------------------------------------------- battery math */

/* LiFePO4 voltage curve: 42 V at 0 %, 54.6 V at 100 % */
static double soc_to_voltage(double soc)
{
    return 42.0 + (soc / 100.0) * 12.6;
}

static void init_batteries(Battery *bats, int num, int modules_per)
{
    for (int b = 0; b < num; b++)
    {
        bats[b].id          = b;
        bats[b].num_modules = modules_per;
        double total_soc = 0.0, total_v = 0.0;
        for (int m = 0; m < modules_per; m++)
        {
            /* stagger each battery and module slightly so they are not identical */
            double soc = 100.0 - b * 5.0 - m * 0.5;
            bats[b].modules[m].soc     = soc;
            bats[b].modules[m].voltage = soc_to_voltage(soc);
            total_soc += soc;
            total_v   += bats[b].modules[m].voltage;
        }
        bats[b].soc     = total_soc / modules_per;
        bats[b].voltage = total_v   / modules_per;
    }
}

static void update_batteries(Battery *bats, int num, double delta_soc)
{
    for (int b = 0; b < num; b++)
    {
        double total_soc = 0.0, total_v = 0.0;
        for (int m = 0; m < bats[b].num_modules; m++)
        {
            bats[b].modules[m].soc -= delta_soc;
            if (bats[b].modules[m].soc < 0.0) bats[b].modules[m].soc = 0.0;
            bats[b].modules[m].voltage = soc_to_voltage(bats[b].modules[m].soc);
            total_soc += bats[b].modules[m].soc;
            total_v   += bats[b].modules[m].voltage;
        }
        bats[b].soc     = total_soc / bats[b].num_modules;
        bats[b].voltage = total_v   / bats[b].num_modules;
    }
}

/* ------------------------------------------------------------ MQTT helpers */

static void pub(struct mosquitto *mosq, const char *topic, double val)
{
    char buf[64];
    snprintf(buf, sizeof(buf), "%.2f", val);
    mosquitto_publish(mosq, NULL, topic, (int)strlen(buf), buf, 0, true);
}

static void publish_all(struct mosquitto *mosq, Battery *bats, int num,
                         const char *prefix)
{
    char topic[512];
    for (int b = 0; b < num; b++)
    {
        snprintf(topic, sizeof(topic), "%s/%d/voltage", prefix, b);
        pub(mosq, topic, bats[b].voltage);

        snprintf(topic, sizeof(topic), "%s/%d/soc", prefix, b);
        pub(mosq, topic, bats[b].soc);

        for (int m = 0; m < bats[b].num_modules; m++)
        {
            snprintf(topic, sizeof(topic), "%s/%d/module/%d/voltage", prefix, b, m);
            pub(mosq, topic, bats[b].modules[m].voltage);

            snprintf(topic, sizeof(topic), "%s/%d/module/%d/soc", prefix, b, m);
            pub(mosq, topic, bats[b].modules[m].soc);
        }
    }
}

/* --------------------------------------------------------------------- main */

static void usage(void)
{
    fprintf(stderr,
        "usage: battery2mqtt [-c config] [-h]\n"
        "  -c <path>   config file (default: battery2mqtt.conf)\n"
        "  -h          show this help\n");
}

int main(int argc, char *argv[])
{
    Config cfg;
    default_config(&cfg);

    const char *config_path = "battery2mqtt.conf";
    for (int i = 1; i < argc; i++)
    {
        if (!strcmp(argv[i], "-c") && i + 1 < argc)
        {
            config_path = argv[++i];
        }
        else if (!strcmp(argv[i], "-h"))
        {
            usage();
            return 0;
        }
    }

    if (load_config(&cfg, config_path) != 0)
        fprintf(stderr, "warning: %s not found, using defaults\n", config_path);

    Battery bats[MAX_BATTERIES];
    init_batteries(bats, cfg.num_batteries, cfg.modules_per_battery);

    mosquitto_lib_init();
    struct mosquitto *mosq = mosquitto_new("battery2mqtt", true, NULL);
    if (!mosq) { fprintf(stderr, "error: mosquitto_new failed\n"); return 1; }

    if (mosquitto_connect(mosq, cfg.mqtt_host, cfg.mqtt_port, 60) != MOSQ_ERR_SUCCESS)
    {
        fprintf(stderr, "error: cannot connect to %s:%d\n", cfg.mqtt_host, cfg.mqtt_port);
        return 1;
    }
    mosquitto_loop_start(mosq);

    printf("connected to %s:%d — publishing every %d ms\n",
           cfg.mqtt_host, cfg.mqtt_port, cfg.interval_ms);

    const double discharge_per_tick = 0.05;   /* % SoC per tick */

    while (1)
    {
        publish_all(mosq, bats, cfg.num_batteries, cfg.topic_prefix);
        update_batteries(bats, cfg.num_batteries, discharge_per_tick);
        usleep(cfg.interval_ms * 1000);
    }

    mosquitto_loop_stop(mosq, true);
    mosquitto_destroy(mosq);
    mosquitto_lib_cleanup();
    return 0;
}
