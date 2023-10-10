import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/domain/liquidity_option.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/features/wallet/onboarding/liquidity_card.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:provider/provider.dart';

class OnboardingScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "onboarding";

  const OnboardingScreen({super.key});

  @override
  State<OnboardingScreen> createState() => _OnboardingScreenState();
}

class _OnboardingScreenState extends State<OnboardingScreen> {
  Amount? amount;
  bool valid = true;

  Amount minDeposit = Amount(0);
  Amount? maxDeposit;

  List<LiquidityOption> liquidityOptions = [];

  final _formKey = GlobalKey<FormState>();

  /// Estimated fees for receiving
  ///
  /// These fees have to be added on top of the receive amount because they are collected after receiving the funds.
  Amount? feeEstimate;

  @override
  void initState() {
    final bridge.Config config = context.read<bridge.Config>();
    amount = config.network == "regtest" ? Amount(100000) : null;

    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final channelInfoService = context.read<ChannelInfoService>();

    return Scaffold(
      appBar: AppBar(title: const Text("Fund 10101 Wallet")),
      body: Form(
        key: _formKey,
        child: GestureDetector(
          onTap: () {
            FocusScope.of(context).requestFocus(FocusNode());
          },
          behavior: HitTestBehavior.opaque,
          child: ScrollableSafeArea(
            child: FutureBuilder<List<LiquidityOption>>(
                future: _getLiquidityOptions(channelInfoService),
                builder: (BuildContext context, AsyncSnapshot<List<LiquidityOption>> config) {
                  if (!config.hasData) {
                    return const Center(
                        child: SizedBox(width: 20, height: 20, child: CircularProgressIndicator()));
                  }

                  final liquidityCards = config.data!
                      .map((l) => LiquidityCard(
                          liquidityOptionId: l.liquidityOptionId,
                          title: l.title,
                          tradeUpTo: l.tradeUpTo,
                          fee: l.fee,
                          minDeposit: l.minDeposit,
                          maxDeposit: l.maxDeposit,
                          amount: amount,
                          enabled: valid,
                          onTap: (min, max) {
                            setState(() {
                              minDeposit = min;
                              maxDeposit = max;
                            });
                            _formKey.currentState?.validate();
                          }))
                      .toList();

                  return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                    Padding(
                      padding: const EdgeInsets.fromLTRB(20, 32, 20, 0),
                      child: Row(
                        children: [
                          Expanded(
                            child: AmountInputField(
                                value: amount ?? Amount(0),
                                hint: "e.g. ${Amount(100000)}",
                                label: "Amount",
                                isLoading: false,
                                onChanged: (value) {
                                  if (value.isEmpty) {
                                    return;
                                  }

                                  setState(() {
                                    amount = Amount.parseAmount(value);
                                    minDeposit = Amount.zero();
                                    maxDeposit = null;
                                  });
                                  valid = _formKey.currentState?.validate() ?? false;
                                },
                                validator: (value) {
                                  if (value == null) {
                                    return "Enter receive amount";
                                  }

                                  final amt = Amount.parseAmount(value);
                                  if (amt.sats < minDeposit.sats) {
                                    return "Min amount to receive is ${formatSats(minDeposit)}";
                                  }

                                  if (maxDeposit != null && amt.sats > maxDeposit!.sats) {
                                    return "Max amount to receive is ${formatSats(maxDeposit!)}";
                                  }

                                  return null;
                                }),
                          ),
                        ],
                      ),
                    ),
                    Container(
                      padding: const EdgeInsets.fromLTRB(20, 20, 20, 0),
                      child: const Text("Choose your liquidity requirement from the options below!",
                          style: TextStyle(fontSize: 16)),
                    ),
                    Expanded(
                      child: Padding(
                        padding: const EdgeInsets.fromLTRB(20, 20, 20, 0),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          mainAxisAlignment: MainAxisAlignment.start,
                          children: liquidityCards,
                        ),
                      ),
                    )
                  ]);
                }),
          ),
        ),
      ),
    );
  }

  Future<List<LiquidityOption>> _getLiquidityOptions(ChannelInfoService channelInfoService) async {
    // fetch only active liquidity options
    List<LiquidityOption> liquidityOptions = await channelInfoService.getLiquidityOptions(true);

    var completer = Completer<List<LiquidityOption>>();
    completer.complete(liquidityOptions);

    return completer.future;
  }
}
