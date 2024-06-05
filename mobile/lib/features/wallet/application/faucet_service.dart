import 'dart:convert';
import 'dart:math';

import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:http/http.dart' as http;

enum Layer { onchain, lightning }

class FaucetService {
  /// Pay the provided invoice with our faucet
  Future<void> payInvoiceWithFaucet(
      String bip21Uri, Amount? invoiceAmount, String network, Layer layer) async {
    final split = bip21Uri.split(":");
    final addressAndMaybeAmount = split[1].split("?");
    logger.i("Funding $addressAndMaybeAmount");
    final address = addressAndMaybeAmount[0];
    final amount = invoiceAmount?.btc ?? 1.0;

    logger.i("Funding $address with $amount");

    switch (network) {
      case "regtest":
        switch (layer) {
          case Layer.onchain:
            await payWith10101Faucet(address, amount);
          case Layer.lightning:
            throw Exception("We don't have a regtest faucet for LN");
        }
        break;
      case "signet":
        switch (layer) {
          case Layer.onchain:
            await payWithMutinyOnChainFaucet(address, amount);
          case Layer.lightning:
            await payWithMutinyLightningFaucet(address);
        }

        break;
      default:
        throw Exception("Invalid network provided $network. Only regtest or signet supported");
    }
  }

  Future<void> payWith10101Faucet(String address, double amountBtc) async {
    // Faucet env variable needs to be set for local testing, otherwise we will fail here
    if (!const bool.hasEnvironment("REGTEST_FAUCET")) {
      throw Exception("Could not fund address. REGTEST_FAUCET not set");
    }

    String faucet =
        const String.fromEnvironment("REGTEST_FAUCET", defaultValue: "http://localhost:8080");

    final data = {
      'jsonrpc': '1.0',
      'method': 'sendtoaddress',
      'params': [address, "$amountBtc"],
    };
    final encodedData = json.encode(data);

    final response = await http.post(
      Uri.parse('$faucet/bitcoin'),
      headers: <String, String>{'Content-Type': 'text/plain'},
      body: encodedData,
    );

    if (response.statusCode != 200 || !response.body.contains('"error":null')) {
      throw Exception("Funding failed: Received ${response.statusCode} ${response.body}");
    } else {
      logger.i("Funding succeeded: ${response.body}");
    }

    {
      final data = {
        'jsonrpc': '1.0',
        'method': 'generatetoaddress',
        // a random address
        'params': [7, "bcrt1qlzarjr7s5fs983q7hcfuhnensrp9cs0n8gkacz"],
      };

      final encodedData = json.encode(data);

      final response = await http.post(
        Uri.parse('$faucet/bitcoin'),
        headers: <String, String>{'Content-Type': 'text/plain'},
        body: encodedData,
      );

      if (response.statusCode != 200 || !response.body.contains('"error":null')) {
        throw Exception("Mining blocks failed: received ${response.statusCode} ${response.body}");
      } else {
        logger.i("Mining blocks succeeded: ${response.body}");
      }
    }
  }

  Future<void> payWithMutinyOnChainFaucet(String address, double amountBtc) async {
    final url = Uri.parse('https://faucet.mutinynet.com/api/onchain');
    final headers = {
      'Content-Type': 'application/json',
      'Origin': 'https://faucet.mutinynet.com',
    };
    final body = jsonEncode({
      'sats': min(amountBtc * 100000000, 10000000).toInt(),
      'address': address,
    });

    try {
      final response = await http.post(
        url,
        headers: headers,
        body: body,
      );

      if (response.statusCode == 200) {
        logger.i('Funding successful ${response.body}');
      } else {
        logger.e('Request failed with status: ${response.statusCode} ${response.body}');
        throw Exception("Failed funding address ${response.statusCode} ${response.body}");
      }
    } catch (e) {
      throw Exception("Failed funding address ${e.toString()}");
    }
  }

  Future<void> payWithMutinyLightningFaucet(String bolt11) async {
    final url = Uri.parse('https://faucet.mutinynet.com/api/lightning');
    final headers = {
      'Content-Type': 'application/json',
      'Origin': 'https://faucet.mutinynet.com',
    };
    final body = jsonEncode({'bolt11': bolt11});

    try {
      final response = await http.post(url, headers: headers, body: body);

      if (response.statusCode == 200) {
        logger.i('Funding successful ${response.body}');
      } else {
        logger.e('Request failed with status: ${response.statusCode} ${response.body}');
        throw Exception("Failed funding address ${response.statusCode} ${response.body}");
      }
    } catch (e) {
      throw Exception("Failed funding address ${e.toString()}");
    }
  }
}
