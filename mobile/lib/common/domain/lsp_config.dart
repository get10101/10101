import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/domain/liquidity_option.dart';

class LspConfig {
  final int contractTxFeeRate;
  final List<LiquidityOption> liquidityOptions;

  LspConfig({required this.contractTxFeeRate, required this.liquidityOptions});

  static LspConfig fromApi(bridge.LspConfig config) {
    return LspConfig(
      contractTxFeeRate: config.contractTxFeeRate,
      liquidityOptions: config.liquidityOptions.map((lo) => LiquidityOption.from(lo)).toList(),
    );
  }

  static bridge.LspConfig apiDummy() {
    return const bridge.LspConfig(contractTxFeeRate: 0, liquidityOptions: []);
  }
}
