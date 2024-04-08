import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/domain/liquidity_option.dart';

class TenTenOneConfig {
  final List<LiquidityOption> liquidityOptions;

  TenTenOneConfig({required this.liquidityOptions});

  static TenTenOneConfig fromApi(bridge.TenTenOneConfig config) {
    return TenTenOneConfig(
      liquidityOptions: config.liquidityOptions.map((lo) => LiquidityOption.from(lo)).toList(),
    );
  }

  static bridge.TenTenOneConfig apiDummy() {
    return const bridge.TenTenOneConfig(
        liquidityOptions: [], minQuantity: 1, maintenanceMarginRate: 0.1);
  }
}
