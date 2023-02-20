#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
typedef struct _Dart_Handle* Dart_Handle;

typedef struct DartCObject DartCObject;

typedef int64_t DartPort;

typedef bool (*DartPostCObjectFnType)(DartPort port_id, void *message);

typedef struct DartCObject *WireSyncReturn;

typedef struct wire_OrderType_Market {

} wire_OrderType_Market;

typedef struct wire_OrderType_Limit {
  double price;
} wire_OrderType_Limit;

typedef union OrderTypeKind {
  struct wire_OrderType_Market *Market;
  struct wire_OrderType_Limit *Limit;
} OrderTypeKind;

typedef struct wire_OrderType {
  int32_t tag;
  union OrderTypeKind *kind;
} wire_OrderType;

typedef struct wire_NewOrder {
  double leverage;
  double quantity;
  int32_t contract_symbol;
  int32_t direction;
  struct wire_OrderType *order_type;
} wire_NewOrder;

typedef struct wire_uint_8_list {
  uint8_t *ptr;
  int32_t len;
} wire_uint_8_list;

void store_dart_post_cobject(DartPostCObjectFnType ptr);

Dart_Handle get_dart_object(uintptr_t ptr);

void drop_dart_object(uintptr_t ptr);

uintptr_t new_dart_opaque(Dart_Handle handle);

intptr_t init_frb_dart_api_dl(void *obj);

void wire_init_logging(int64_t port_);

WireSyncReturn wire_calculate_margin(double price, double quantity, double leverage);

WireSyncReturn wire_calculate_quantity(double price, uint64_t margin, double leverage);

WireSyncReturn wire_calculate_liquidation_price(double price, double leverage, int32_t direction);

void wire_submit_order(int64_t port_, struct wire_NewOrder *order);

void wire_get_order(int64_t port_, struct wire_uint_8_list *id);

void wire_get_orders(int64_t port_);

void wire_subscribe(int64_t port_);

void wire_run(int64_t port_, struct wire_uint_8_list *app_dir);

WireSyncReturn wire_get_new_address(void);

void wire_open_channel(int64_t port_);

void wire_create_invoice(int64_t port_);

void wire_send_payment(int64_t port_, struct wire_uint_8_list *invoice);

struct wire_NewOrder *new_box_autoadd_new_order_0(void);

struct wire_OrderType *new_box_order_type_0(void);

struct wire_uint_8_list *new_uint_8_list_0(int32_t len);

union OrderTypeKind *inflate_OrderType_Limit(void);

void free_WireSyncReturn(WireSyncReturn ptr);

static int64_t dummy_method_to_enforce_bundling(void) {
    int64_t dummy_var = 0;
    dummy_var ^= ((int64_t) (void*) wire_init_logging);
    dummy_var ^= ((int64_t) (void*) wire_calculate_margin);
    dummy_var ^= ((int64_t) (void*) wire_calculate_quantity);
    dummy_var ^= ((int64_t) (void*) wire_calculate_liquidation_price);
    dummy_var ^= ((int64_t) (void*) wire_submit_order);
    dummy_var ^= ((int64_t) (void*) wire_get_order);
    dummy_var ^= ((int64_t) (void*) wire_get_orders);
    dummy_var ^= ((int64_t) (void*) wire_subscribe);
    dummy_var ^= ((int64_t) (void*) wire_run);
    dummy_var ^= ((int64_t) (void*) wire_get_new_address);
    dummy_var ^= ((int64_t) (void*) wire_open_channel);
    dummy_var ^= ((int64_t) (void*) wire_create_invoice);
    dummy_var ^= ((int64_t) (void*) wire_send_payment);
    dummy_var ^= ((int64_t) (void*) new_box_autoadd_new_order_0);
    dummy_var ^= ((int64_t) (void*) new_box_order_type_0);
    dummy_var ^= ((int64_t) (void*) new_uint_8_list_0);
    dummy_var ^= ((int64_t) (void*) inflate_OrderType_Limit);
    dummy_var ^= ((int64_t) (void*) free_WireSyncReturn);
    dummy_var ^= ((int64_t) (void*) store_dart_post_cobject);
    dummy_var ^= ((int64_t) (void*) get_dart_object);
    dummy_var ^= ((int64_t) (void*) drop_dart_object);
    dummy_var ^= ((int64_t) (void*) new_dart_opaque);
    return dummy_var;
}