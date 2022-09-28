# QuorumOS

QuorumOS is a trusted computation layer for hosting enclave apps at modern cloud scale. The OS architecture is based on the principle that threshold, or _quorum_, of actors must coordinate to provision a secure compute environment with sensitive application logic and secret material; no single actor can unilaterally provision the environment or secret material.

Concretely, QuorumOS is designed to boot in an enclave by attesting to the enclave configuration, reconstructing a Quorum Key and then launching a single enclave app that can leverage the Quorum Key to encrypt and authenticate data.

The consensus on environment configuration is coordinated through the Manifest document which describes, among other things, the enclave image configuration, application CLI arguments, public Quorum Key, and Quorum Set. During the bootstrapping process, a threshold of Quorum Members will attest to the enclaves configuration against the Manifest out of band and then post their respective Quorum Key share. See the [instance provision](#instance-provision) section for details.

The Quorum Key itself can be used by QuorumOS and enclave apps to encrypt and authenticate data.

## System requirements

- openssl >= 1.1.0

## Development

### Submitting a PR

Before a PR can be merged it must:

Be formatted

```bash
make lint
```

And pass all tests

```bash
make test-all
```

### View the docs

In the root of this project run

```bash
cargo doc --open
```

## Conceptual

### Major Components

#### Enclave

- houses server for listening to the Host
- contains logic for quorum key genesis, booting, and running a Enclave App
- see crate `qos_core`

#### Host

- EC2 instance housing the nitro enclave
- has client for talking to nitro enclave
- has server for incoming request from outside world
- see crate `qos_host`

#### Client

- anything making request to host
- see crate `qos_client`

### Key Terms

#### Quorum Key

An asymmetric key used to uniquely authenticate and encrypt data. This key should only ever be reconstituted inside of an enclave. Additionally, the full provisioning of the Quorum Key concludes the attestation flow launches, at which point QuorumOS pivots to launching the specified enclave app. At rest outside of the enclave, the key is intended to be stored across shares using shamir's secret sharing.

#### Quorum Member

An entity that is a member of the Quorum Set and holds a share of the Quorum Key.

#### Quorum Set

The collection of members whom each hold shares of the Quorum Key and can authorize certain QOS actions. A threshold of these members shares is required to reconstruct the Quorum Key

#### Personal Key

A key held by a Quorum Member that can be used to encrypt/decrypt their share of the Quorum Key.

### Personal Setup Key

A key held by a Quorum Member that can be used to encrypt secret data before a QuorumOS instance has been fully bootstrapped.

#### Ephemeral Key

An asymmetric key that is generated by a QuorumOS instance immediately after boot. Once Quorum Members are able to verify the integrity of a QuorumOS instance, they encrypt their Quorum Key shares to the Ephemeral Key and submit to the instance for reconstruction.

#### Manifest

A file that contains the static configuration to launch an instance of QuorumOS. The composition of the Manifest is attested to in the boot process. All Quorum Members will agree to the Manifest by signing it (QuorumOS should reject a submitted manifest if it has less than threshold signatures.)

### Node

A single machine compute instance running QuorumOS.

#### Namespace

A group of QuorumOS Nodes running the same Enclave App and using the same Quorum Key. A Namespace contains many live Nodes all with the same Quorum Key and enclave app. Some of these nodes could be using different Manifests and different versions of the same enclave app.

### Pivot / Enclave App

The application QuorumOS pivots to once it finishes booting. This applications binary hash and CLI arguments are specified in the Manifest file.

### Instance Provision

Immediately after a valid Manifest is loaded into QuorumOS, the instance will generate an Ephemeral Key. This key is specific to a particular individual machine and, after successfully verifying the machine image and metadata contained in the manifest file, will be used by the Quorum Members to post their shares into the machine.

Prior to posting their share to the machine, Quorum Members use a variety of cryptographic attestations and social coordination to determine if it is appropriate to provision a particular instance of QuorumOS with a given secret.

Upon successful verification of the attestation outputs, each member will encrypt their share to an Ephemeral Key. Once threshold shares have been collected by the instance, it will use Shamir’s Secret Sharing to reconstruct the Quorum Key.

#### Remote Attestation

The purpose of remote attestation is to prove that an environment is running a particular piece of software. In the case of AWS Nitro Enclaves, an enclave can uniquely asks the Nitro Security Module (NSM) for an attestation document containing details of the enclave. This document is signed by the Nitro Attestation PKI and is tied back to the AWS Nitro Attestation PKI Root.

As defined in the [AWS documentation](https://docs.aws.amazon.com/enclaves/latest/user/verify-root.html) the instance can request the Nitro Security Module (NSM) to produce an attestation document on its behalf. Additionally, the attestation document contains two fields that can be modified by the enclave itself. The attestation document request contains the Ephemeral Key and the hash of manifest so Quorum Set members can verify the data is correct.

Before provisioning a namespace with the Quorum Key, a Quorum Set will use the output of the attestation process against the enclave to verify that the enclave is running the expected version of QuorumOS and that that instance is configured in the expected manner as to warrant being provisioned with that Quorum Key.
